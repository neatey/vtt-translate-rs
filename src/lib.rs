use crate::translate::{Language, TranslationClient};
use crate::vtt::Vtt;
use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub mod translate;
pub mod vtt;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The VTT file to translate.
    #[arg(long, short = 'f')]
    source_vtt_file: PathBuf,

    /// The output translated VTT file to write (whichwill be overwritten). Defaults to an auto-generated filename based on source_vtt_file and target_language.
    #[arg(long)]
    target_vtt_file: Option<PathBuf>,

    /// Language the source VTT file is in. If not specified then we attempt to auto-detect it.
    #[arg(long)]
    source_language: Option<Language>,

    /// Language to translate the VTT file to.
    #[arg(long, short = 'l', default_value_t = Language::Fa)]
    target_language: Language,

    /// Key for the Azure Translation resource.
    #[arg(long, env = "AZURE_TRANSLATION_RESOURCE_KEY")]
    azure_resource_key: String,

    /// Azure region the Translation resource is running in.
    #[arg(long, env = "AZURE_TRANSLATION_RESOURCE_REGION")]
    azure_resource_region: String,
}

#[derive(Debug, Clone)]
struct ChunkDesc {
    block_num: usize,
    line_num: usize,
    chunk_len: usize,
}

type Sentence = (Vec<ChunkDesc>, String);

impl From<crate::translate::Direction> for crate::vtt::Direction {
    fn from(value: crate::translate::Direction) -> Self {
        match value {
            crate::translate::Direction::Ltr => crate::vtt::Direction::Ltr,
            crate::translate::Direction::Rtl => crate::vtt::Direction::Rtl,
        }
    }
}

fn recontruct_sentences(vtt: &Vtt) -> Vec<Sentence> {
    let mut all_sentences: Vec<Sentence> = vec![];
    let mut this_sentence = "".to_string();
    let mut this_sentence_chunk_descs: Vec<ChunkDesc> = vec![];

    for (block_num, block) in vtt.blocks.iter().enumerate() {
        for (line_num, text_line) in block.text_lines.clone().into_iter().enumerate() {
            let mut chunks = text_line.trim().split('.').peekable();
            while let Some(chunk) = chunks.next() {
                // A trailing fullstop results in an empty chunk, which we can ignore
                if !chunk.is_empty() {
                    let chunk = chunk.trim();
                    let chunk_desc = ChunkDesc {
                        block_num,
                        line_num,
                        chunk_len: chunk.len(),
                    };

                    this_sentence.push_str(chunk);
                    this_sentence_chunk_descs.push(chunk_desc);

                    if chunks.peek().is_some() {
                        // This is the end of a sentence
                        this_sentence.push('.');
                        all_sentences.push((this_sentence_chunk_descs, this_sentence));
                        this_sentence = "".to_string();
                        this_sentence_chunk_descs = vec![];
                    } else {
                        // Add a trailing space to unfinished sentence
                        this_sentence.push(' ');
                    }
                }
            }
        }
    }
    all_sentences
}

fn update_vtt(vtt: &mut Vtt, sentences: &Vec<Sentence>) {
    // Initialize the vtt block text lines with empty strings (deleting any existing ones)
    vtt.blocks.iter_mut().for_each(|vb| {
        vb.text_lines = vec!["".to_string(); vb.text_lines.len()];
    });

    // Iterate through all the sentences and update the vtt blocks with the new text
    for sentence in sentences {
        let new_text = sentence.1.clone();
        let mut new_text_words = new_text.split(' ');
        let mut next_word = new_text_words.next();

        // Calculate the total length of all chunks in the original text
        let mut total_chunks_len = 0;
        sentence
            .0
            .clone()
            .into_iter()
            .for_each(|cd| total_chunks_len += cd.chunk_len);

        // Iterate through the chunks and add equivalent sized portions of the new text to the vtt blocks
        let mut chunk_descs = sentence.0.clone().into_iter().peekable();
        while let Some(chunk_desc) = chunk_descs.next() {
            // Calculate the desired number of characters in this chunk
            let new_chunk_size = chunk_desc.chunk_len * new_text.len() / total_chunks_len;

            // Add words to this chunk until it is close to or greater than the desired length, or there are no later chunks to add the remaining words to
            let mut new_chunk_text = "".to_string();
            while (new_chunk_text.is_empty()
                || new_chunk_text.len() + 3 <= new_chunk_size
                || chunk_descs.peek().is_none())
                && next_word.is_some()
            {
                if !next_word.unwrap().is_empty() {
                    new_chunk_text += next_word.unwrap();
                    new_chunk_text += " ";
                }
                next_word = new_text_words.next();
            }

            // Add this chunk to the vtt block, including a preceeding space if necessary
            if !vtt.blocks[chunk_desc.block_num].text_lines[chunk_desc.line_num].is_empty() {
                vtt.blocks[chunk_desc.block_num].text_lines[chunk_desc.line_num] += " ";
            }
            vtt.blocks[chunk_desc.block_num].text_lines[chunk_desc.line_num] += &new_chunk_text;
        }
    }
}

fn default_target_filename(
    source_filename: &Path,
    source_language: Language,
    target_language: Language,
) -> PathBuf {
    let directory = source_filename.parent().unwrap_or(Path::new(""));
    let stem = source_filename
        .file_stem()
        .unwrap_or(OsStr::new(""))
        .to_str()
        .unwrap();
    let extension = source_filename
        .extension()
        .unwrap_or(OsStr::new(""))
        .to_str()
        .unwrap();

    // Filename regex to match "(prefix)-(language)" where
    // - language is optional
    // - language matches the source language of the VTT
    // - the source language may have been identified as e.g. "en" when the filename is actually "en-GB".
    let filename_re: Regex = Regex::new(&format!(
        "^{}{}$",
        r"(?P<prefix>.+?)",
        r"(?P<language>-(?i)".to_owned() + &source_language.to_string() + "(-[A-Za-z]{2})?)?"
    ))
    .unwrap();

    // Construct the target filename as "(prefix)-(target language)"
    let mut target_filename = filename_re
        .captures(stem)
        .map(|cap| {
            format!(
                "{}-{}",
                cap.name("prefix")
                    .map(|p| p.as_str())
                    .unwrap_or("vtt-translate"),
                target_language,
            )
        })
        .unwrap_or("vtt-translate-output.vtt".to_string());
    if !extension.is_empty() {
        target_filename.push_str(&format!(".{}", extension));
    }
    directory.join(target_filename)
}

pub async fn run(args: Cli) -> Result<()> {
    // Parse the vtt file
    println!("Parsing VTT file {:?}...", args.source_vtt_file);
    let from_vtt = Vtt::parse(&args.source_vtt_file)?;
    
    // Scan the Vec of Blocks and convert to a Vec of whole sentences
    let mut all_sentences = recontruct_sentences(&from_vtt);

    // Translate the full sentences
    let translation_client =
        TranslationClient::new(args.azure_resource_key, args.azure_resource_region);
    let from_sentences = all_sentences
        .clone()
        .into_iter()
        .map(|(_cds, s)| s)
        .collect::<Vec<String>>();
    println!("Calling Azure translation API...");
    let (source_language, direction, to_sentences) = translation_client
        .translate(from_sentences, args.source_language, args.target_language)
        .await?;
    println!("Identified source language as \"{}\"...", source_language);
    println!(
        "Text direction for target language {} is {:?}...",
        args.target_language, direction
    );
    all_sentences.iter_mut().enumerate().for_each(|(n, s)| {
        s.1 = to_sentences[n].clone();
    });

    // Fill the translated sentences back into the vtt blocks
    let mut to_vtt = from_vtt.clone();
    update_vtt(&mut to_vtt, &all_sentences);

    // Write the translated vtt file
    let target_vtt_file = match args.target_vtt_file {
        Some(target_vtt_file) => target_vtt_file,
        None => {
            default_target_filename(&args.source_vtt_file, source_language, args.target_language)
        }
    };
    println!("Writing translated VTT file to {:?}...", target_vtt_file);
    to_vtt
        .write(&target_vtt_file, crate::vtt::Direction::from(direction))
        .with_context(|| format!("Failed to write to VTT file {:?}", target_vtt_file))?;

    println!("Done");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_target_file_stem() {
        assert_eq!(
            default_target_filename(&Path::new("stem-en-GB.ext"), Language::EnGB, Language::Fa),
            PathBuf::from("stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(&Path::new("stem-en-GB"), Language::EnGB, Language::Fa),
            PathBuf::from("stem-fa")
        );
        assert_eq!(
            default_target_filename(&Path::new(".stem-en-GB"), Language::EnGB, Language::Fa),
            PathBuf::from(".stem-fa")
        );
        assert_eq!(
            default_target_filename(&Path::new(".stem-en-GB.ext"), Language::EnGB, Language::Fa),
            PathBuf::from(".stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(&Path::new("stem"), Language::EnGB, Language::Fa),
            PathBuf::from("stem-fa")
        );
        assert_eq!(
            default_target_filename(
                &Path::new("stem-more-stem-en-GB"),
                Language::EnGB,
                Language::Fa
            ),
            PathBuf::from("stem-more-stem-fa")
        );
        assert_eq!(
            default_target_filename(
                &Path::new("stem-more-stem.ext"),
                Language::EnGB,
                Language::Fa
            ),
            PathBuf::from("stem-more-stem-fa.ext")
        );
    }

    #[test]
    fn test_default_target_file_language() {
        assert_eq!(
            default_target_filename(&Path::new("stem-en-gb.ext"), Language::EnGB, Language::Fa),
            PathBuf::from("stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(&Path::new("stem-en-GB.ext"), Language::En, Language::Fa),
            PathBuf::from("stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(&Path::new("stem-en-us.ext"), Language::En, Language::Fa),
            PathBuf::from("stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(&Path::new("stem-en.ext"), Language::En, Language::Fa),
            PathBuf::from("stem-fa.ext")
        );
    }
    #[test]
    fn test_default_target_directory() {
        assert_eq!(
            default_target_filename(
                &Path::new("/directory/stem.ext"),
                Language::EnGB,
                Language::Fa
            ),
            PathBuf::from("/directory/stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(&Path::new("./stem.ext"), Language::EnGB, Language::Fa),
            PathBuf::from("./stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(
                &Path::new("./directory/stem.ext"),
                Language::EnGB,
                Language::Fa
            ),
            PathBuf::from("./directory/stem-fa.ext")
        );
        assert_eq!(
            default_target_filename(
                &Path::new("../directory/stem.ext"),
                Language::EnGB,
                Language::Fa
            ),
            PathBuf::from("../directory/stem-fa.ext")
        );
    }
}
