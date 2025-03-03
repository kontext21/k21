use clap::ValueEnum;
use serde::Serialize;
use std::fmt;
use image::DynamicImage;
use rusty_tesseract::{Args, DataOutput, Image};
use std::{collections::HashMap, sync::Arc};


pub fn perform_ocr_tesseract(
    image: &DynamicImage
) -> String {
    let language_string = "eng".to_string();

    let args = Args {
        lang: language_string,
        config_variables: HashMap::from([("tessedit_create_tsv".into(), "1".into())]),
        dpi: Some(600), // 150 is a balanced option, 600 seems faster surprisingly, the bigger the number the more granualar result
        psm: Some(1), // PSM 1: Automatic page segmentation with OSD. PSM 3: Automatic page segmentation with OSD
        oem: Some(1), //1: Neural nets LSTM engine only,    3: Default, based on what is available. (Default)
    };

    let ocr_image = Image::from_dynamic_image(image).unwrap();

    // Extract data output
    let data_output = rusty_tesseract::image_to_data(&ocr_image, &args).unwrap();
    // let tsv_output = data_output_to_tsv(&data_output);

    // Extract text from data output
    // let text = data_output_to_text(&data_output);
    // let json_output = data_output_to_json(&data_output);

    // let overall_confidence = calculate_overall_confidence(&data_output);

    // (text, json_output, Some(overall_confidence))
    data_output_to_text(&data_output)
}

fn data_output_to_text(data_output: &DataOutput) -> String {
    let mut text = String::new();
    for record in &data_output.data {
        if !record.text.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&record.text);
        }
    }
    text
}

fn data_output_to_json(data_output: &DataOutput) -> String {
    let mut lines: Vec<HashMap<String, String>> = Vec::new();
    let mut current_line = String::new();
    let mut current_conf = 0.0;
    let mut word_count = 0;
    let mut last_word_num = 0;

    for record in &data_output.data {
        if record.word_num == 0 && !current_line.is_empty() {
            let avg_conf = current_conf / word_count as f32;
            let mut line_data = HashMap::new();
            line_data.insert("text".to_string(), current_line.clone());
            line_data.insert("confidence".to_string(), format!("{:.2}", avg_conf));
            line_data.insert(
                "line_position".to_string(),
                format!(
                    "level{}page_num{}block_num{}par_num{}line_num{}",
                    record.level,
                    record.page_num,
                    record.block_num,
                    record.par_num,
                    record.line_num
                ),
            );
            lines.push(line_data);
            current_line.clear();
            current_conf = 0.0;
            word_count = 0;
        }
        if record.word_num > last_word_num {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(&record.text);
            current_conf += record.conf;
            word_count += 1;
        }
        last_word_num = record.word_num;
    }
    if !current_line.is_empty() {
        let avg_conf = current_conf / word_count as f32;
        let mut line_data = HashMap::new();
        line_data.insert("text".to_string(), current_line);
        line_data.insert("confidence".to_string(), format!("{:.2}", avg_conf));
        lines.push(line_data);
    }

    serde_json::to_string_pretty(&lines).unwrap()
}

fn calculate_overall_confidence(data_output: &DataOutput) -> f64 {
    let total_conf: f32 = data_output.data.iter().map(|record| record.conf).sum();
    let count = data_output.data.len();
    if count > 0 {
        (total_conf / count as f32) as f64
    } else {
        0.0
    }
}


#[derive(ValueEnum, Clone, Debug, Serialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[repr(usize)]
pub enum Language {
    #[clap(name = "english")]
    English,
    #[clap(name = "chinese")]
    Chinese,
    #[clap(name = "german")]
    German,
    #[clap(name = "spanish")]
    Spanish,
    #[clap(name = "russian")]
    Russian,
    #[clap(name = "korean")]
    Korean,
    #[clap(name = "french")]
    French,
    #[clap(name = "japanese")]
    Japanese,
    #[clap(name = "portuguese")]
    Portuguese,
    #[clap(name = "turkish")]
    Turkish,
    #[clap(name = "polish")]
    Polish,
    #[clap(name = "catalan")]
    Catalan,
    #[clap(name = "dutch")]
    Dutch,
    #[clap(name = "arabic")]
    Arabic,
    #[clap(name = "swedish")]
    Swedish,
    #[clap(name = "italian")]
    Italian,
    #[clap(name = "indonesian")]
    Indonesian,
    #[clap(name = "hindi")]
    Hindi,
    #[clap(name = "finnish")]
    Finnish,
    #[clap(name = "hebrew")]
    Hebrew,
    #[clap(name = "ukrainian")]
    Ukrainian,
    #[clap(name = "greek")]
    Greek,
    #[clap(name = "malay")]
    Malay,
    #[clap(name = "czech")]
    Czech,
    #[clap(name = "romanian")]
    Romanian,
    #[clap(name = "danish")]
    Danish,
    #[clap(name = "hungarian")]
    Hungarian,
    #[clap(name = "norwegian")]
    Norwegian,
    #[clap(name = "thai")]
    Thai,
    #[clap(name = "urdu")]
    Urdu,
    #[clap(name = "croatian")]
    Croatian,
    #[clap(name = "bulgarian")]
    Bulgarian,
    #[clap(name = "lithuanian")]
    Lithuanian,
    #[clap(name = "latin")]
    Latin,
    #[clap(name = "malayalam")]
    Malayalam,
    #[clap(name = "welsh")]
    Welsh,
    #[clap(name = "slovak")]
    Slovak,
    #[clap(name = "persian")]
    Persian,
    #[clap(name = "latvian")]
    Latvian,
    #[clap(name = "bengali")]
    Bengali,
    #[clap(name = "serbian")]
    Serbian,
    #[clap(name = "azerbaijani")]
    Azerbaijani,
    #[clap(name = "slovenian")]
    Slovenian,
    #[clap(name = "estonian")]
    Estonian,
    #[clap(name = "macedonian")]
    Macedonian,
    #[clap(name = "nepali")]
    Nepali,
    #[clap(name = "mongolian")]
    Mongolian,
    #[clap(name = "bosnian")]
    Bosnian,
    #[clap(name = "kazakh")]
    Kazakh,
    #[clap(name = "albanian")]
    Albanian,
    #[clap(name = "swahili")]
    Swahili,
    #[clap(name = "galician")]
    Galician,
    #[clap(name = "marathi")]
    Marathi,
    #[clap(name = "punjabi")]
    Punjabi,
    #[clap(name = "sinhala")]
    Sinhala,
    #[clap(name = "khmer")]
    Khmer,
    #[clap(name = "afrikaans")]
    Afrikaans,
    #[clap(name = "belarusian")]
    Belarusian,
    #[clap(name = "gujarati")]
    Gujarati,
    #[clap(name = "amharic")]
    Amharic,
    #[clap(name = "yiddish")]
    Yiddish,
    #[clap(name = "lao")]
    Lao,
    #[clap(name = "uzbek")]
    Uzbek,
    #[clap(name = "faroese")]
    Faroese,
    #[clap(name = "pashto")]
    Pashto,
    #[clap(name = "maltese")]
    Maltese,
    #[clap(name = "sanskrit")]
    Sanskrit,
    #[clap(name = "luxembourgish")]
    Luxembourgish,
    #[clap(name = "myanmar")]
    Myanmar,
    #[clap(name = "tibetan")]
    Tibetan,
    #[clap(name = "tagalog")]
    Tagalog,
    #[clap(name = "assamese")]
    Assamese,
    #[clap(name = "tatar")]
    Tatar,
    #[clap(name = "hausa")]
    Hausa,
    #[clap(name = "javanese")]
    Javanese,
}

impl Language {
    pub fn as_lang_code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
            Language::German => "de",
            Language::Spanish => "es",
            Language::Russian => "ru",
            Language::Korean => "ko",
            Language::French => "fr",
            Language::Japanese => "ja",
            Language::Portuguese => "pt",
            Language::Turkish => "tr",
            Language::Polish => "pl",
            Language::Catalan => "ca",
            Language::Dutch => "nl",
            Language::Arabic => "ar",
            Language::Swedish => "sv",
            Language::Italian => "it",
            Language::Indonesian => "id",
            Language::Hindi => "hi",
            Language::Finnish => "fi",
            Language::Hebrew => "he",
            Language::Ukrainian => "uk",
            Language::Greek => "el",
            Language::Malay => "ms",
            Language::Czech => "cs",
            Language::Romanian => "ro",
            Language::Danish => "da",
            Language::Hungarian => "hu",
            Language::Norwegian => "no",
            Language::Thai => "th",
            Language::Urdu => "ur",
            Language::Croatian => "hr",
            Language::Bulgarian => "bg",
            Language::Lithuanian => "lt",
            Language::Latin => "la",
            Language::Malayalam => "ml",
            Language::Welsh => "cy",
            Language::Slovak => "sk",
            Language::Persian => "fa",
            Language::Latvian => "lv",
            Language::Bengali => "bn",
            Language::Serbian => "sr",
            Language::Azerbaijani => "az",
            Language::Slovenian => "sl",
            Language::Estonian => "et",
            Language::Macedonian => "mk",
            Language::Nepali => "ne",
            Language::Mongolian => "mn",
            Language::Bosnian => "bs",
            Language::Kazakh => "kk",
            Language::Albanian => "sq",
            Language::Swahili => "sw",
            Language::Galician => "gl",
            Language::Marathi => "mr",
            Language::Punjabi => "pa",
            Language::Sinhala => "si",
            Language::Khmer => "km",
            Language::Afrikaans => "af",
            Language::Belarusian => "be",
            Language::Gujarati => "gu",
            Language::Amharic => "am",
            Language::Yiddish => "yi",
            Language::Lao => "lo",
            Language::Uzbek => "uz",
            Language::Faroese => "fo",
            Language::Pashto => "ps",
            Language::Maltese => "mt",
            Language::Sanskrit => "sa",
            Language::Luxembourgish => "lb",
            Language::Myanmar => "my",
            Language::Tibetan => "bo",
            Language::Tagalog => "tl",
            Language::Assamese => "as",
            Language::Tatar => "tt",
            Language::Hausa => "ha",
            Language::Javanese => "jw",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let possible_value = self.to_possible_value().unwrap();
        let action_name = possible_value.get_name();
        write!(f, "{}", action_name)
    }
}

impl PartialEq<&str> for Language {
    fn eq(&self, other: &&str) -> bool {
        self.to_string().as_str() == *other
    }
}

pub const TESSERACT_LANGUAGES: [(&str, &str); 76] = [
    ("eng", "english"),
    ("chi_sim", "chinese"),
    ("deu", "german"),
    ("spa", "spanish"),
    ("rus", "russian"),
    ("kor", "korean"),
    ("fra", "french"),
    ("jpn", "japanese"),
    ("por", "portuguese"),
    ("tur", "turkish"),
    ("pol", "polish"),
    ("cat", "catalan"),
    ("nld", "dutch"),
    ("ara", "arabic"),
    ("swe", "swedish"),
    ("ita", "italian"),
    ("ind", "indonesian"),
    ("hin", "hindi"),
    ("fin", "finnish"),
    ("vie", "vietnamese"),
    ("heb", "hebrew"),
    ("ukr", "ukrainian"),
    ("ell", "greek"),
    ("msa", "malay"),
    ("ces", "czech"),
    ("ron", "romanian"),
    ("dan", "danish"),
    ("hun", "hungarian"),
    ("nor", "norwegian"),
    ("tha", "thai"),
    ("urd", "urdu"),
    ("hrv", "croatian"),
    ("bul", "bulgarian"),
    ("lit", "lithuanian"),
    ("lat", "latin"),
    ("mal", "malayalam"),
    ("cym", "welsh"),
    ("slk", "slovak"),
    ("fas", "persian"),
    ("lav", "latvian"),
    ("ben", "bengali"),
    ("srp", "serbian"),
    ("aze", "azerbaijani"),
    ("slv", "slovenian"),
    ("est", "estonian"),
    ("mkd", "macedonian"),
    ("nep", "nepali"),
    ("mon", "mongolian"),
    ("bos", "bosnian"),
    ("kaz", "kazakh"),
    ("sqi", "albanian"),
    ("swa", "swahili"),
    ("glg", "galician"),
    ("mar", "marathi"),
    ("pan", "punjabi"),
    ("sin", "sinhala"),
    ("khm", "khmer"),
    ("afr", "afrikaans"),
    ("bel", "belarusian"),
    ("guj", "gujarati"),
    ("amh", "amharic"),
    ("yid", "yiddish"),
    ("lao", "lao"),
    ("uzb", "uzbek"),
    ("fo", "faroese"),
    ("pus", "pashto"),
    ("mlt", "maltese"),
    ("san", "sanskrit"),
    ("lb", "luxembourgish"),
    ("mya", "myanmar"),
    ("bod", "tibetan"),
    ("tgl", "tagalog"),
    ("asm", "assamese"),
    ("tat", "tatar"),
    ("hau", "hausa"),
    ("jav", "javanese"),
];
