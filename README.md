# vtt-translate
CLI tool to translate the text in a VTT subtitles file using the Azure Text Translation REST API.

The tool parses the VTT file, converts the raw text to full sentences (for improved translation quality), passes them to the translation API, re-inserts line breaks and writes an output VTT file.

Currently only support English and Farsi. To add support for extra languages, please raise an Issue or make a Pull Request.
```
Usage: vtt-translate [OPTIONS] --source-vtt-file <SOURCE_VTT_FILE> --azure-resource-key <AZURE_RESOURCE_KEY> --azure-resource-region <AZURE_RESOURCE_REGION>

Options:
  -f, --source-vtt-file <SOURCE_VTT_FILE>
          The VTT file to translate
      --target-vtt-file <TARGET_VTT_FILE>
          The output translated VTT file to write (whichwill be overwritten). Defaults to an auto-generated filename based on source_vtt_file and target_language
      --source-language <SOURCE_LANGUAGE>
          Language the source VTT file is in. If not specified then we attempt to auto-detect it [possible values: en, en-gb, fa]
  -l, --target-language <TARGET_LANGUAGE>
          Language to translate the VTT file to [default: fa] [possible values: en, en-gb, fa]
      --azure-resource-key <AZURE_RESOURCE_KEY>
          Key for the Azure Translation resource [env: AZURE_TRANSLATION_RESOURCE_KEY]
      --azure-resource-region <AZURE_RESOURCE_REGION>
          Azure region the Translation resource is running in [env: AZURE_TRANSLATION_RESOURCE_REGION]
  -h, --help
          Print help
  -V, --version
          Print version
```

# Installation (Linux / bash)

## Deploy an Azure Translation resource

```
<az CLI commands...>

export AZURE_TRANSLATION_RESOURCE_KEY=xxx
export AZURE_TRANSLATION_RESOURCE_REGION=xxx
```

## Install vtt-translate

```
cargo build
./target/build/vtt-translate
```