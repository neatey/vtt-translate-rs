use anyhow::{Context, Result};
use regex::Regex;
use std::fs::File;
use std::io::{self, BufRead, Write};

#[derive(Debug, Clone)]
pub struct VttBlock {
    pub _id: String,
    pub timecode: String,
    pub text_lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Vtt {
    pub blocks: Vec<VttBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Vtt {
    pub fn parse<P: AsRef<std::path::Path> + ?Sized + std::fmt::Debug>(path: &P) -> Result<Vtt> {
        let mut vtt = Vtt::default();
        let file =
            File::open(path).with_context(|| format!("Failed to open VTT file {:?}", path))?;
        let lines = io::BufReader::new(file).lines();
        let mut block: Option<VttBlock> = None;

        for line in lines {
            let line = line?;

            if is_blank(&line) || line == "WEBVTT" {
                // Ignore blank lines and header lines
                continue;
            } else if is_new_block(&line) {
                // This line begins a new block - save off the old one
                if let Some(prev_block) = block {
                    vtt.blocks.push(prev_block);
                }
                block = Some(VttBlock {
                    _id: line,
                    timecode: String::from(""),
                    text_lines: vec![],
                });
            } else if is_timecode(&line) {
                // Timecode line - fill in to current block
                block = block.map(|mut b| {
                    b.timecode = line;
                    b
                });
            } else {
                // This is a text line - append to the current block
                block = block.map(|mut b| {
                    b.text_lines.push(line.trim().to_string());
                    b
                });
            }
        }

        if let Some(last_block) = block {
            vtt.blocks.push(last_block);
        }
        Ok(vtt)
    }

    pub fn write<P: AsRef<std::path::Path> + std::fmt::Debug>(
        &self,
        path: &P,
        direction: Direction,
    ) -> Result<()> {
        let mut vtt_file =
            File::create(path).with_context(|| format!("Failed to create VTT file {:?}", path))?;

        writeln!(vtt_file, "WEBVTT\n")?;

        for vtt_block in self.blocks.iter() {
            writeln!(vtt_file, "{}", vtt_block._id)?;
            writeln!(vtt_file, "{}", vtt_block.timecode)?;
            for line in vtt_block.text_lines.iter() {
                let mut line = line.trim().to_string();
                if direction == Direction::Rtl {
                    // If the line starts with a Latin character, add a preceeding RLM
                    if line.chars().next().unwrap_or('a').is_ascii() {
                        line = format!("\u{200F}{line}");
                    }
                    // If the line ends with a Latin character, add a trailing right-left-mark
                    if line.trim().chars().last().unwrap_or('a').is_ascii() {
                        line = format!("{line}\u{200F}");
                    }
                }
                writeln!(vtt_file, "{}", line)?;
            }
            writeln!(vtt_file)?;
        }
        Ok(())
    }
}

fn is_blank(line: &str) -> bool {
    let re = Regex::new(r"^\s*$").unwrap();
    re.is_match(line)
}

fn is_new_block(line: &str) -> bool {
    let re = Regex::new(
        r"^[0-9a-fA-F]{8}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{12}",
    )
    .unwrap();
    re.is_match(line)
}

fn is_timecode(line: &str) -> bool {
    line.contains("-->")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_blank() {
        assert!(is_blank("    "));
    }

    #[test]
    fn test_is_timecode() {
        assert!(is_timecode("00:00:05.020 --> 00:00:08.874"));
    }

    #[test]
    fn test_is_new_block() {
        assert!(is_new_block("f9e6254d-71b5-400f-bdcc-802831ce24f4-0"));
    }
}
