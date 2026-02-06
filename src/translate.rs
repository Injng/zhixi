use rocket_db_pools::Connection;
use rocket_db_pools::sqlx;
use crate::db::Db;

// ========== Algorithmic Title Translation ==========

/// Convert a Chinese numeral string to an integer.
/// Handles: 零=0, 一=1, ..., 十=10, 十一=11, 二十=20, 二十一=21, etc.
fn chinese_num_to_int(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let digit = |c: char| -> Option<u32> {
        match c {
            '零' => Some(0),
            '一' => Some(1),
            '二' => Some(2),
            '三' => Some(3),
            '四' => Some(4),
            '五' => Some(5),
            '六' => Some(6),
            '七' => Some(7),
            '八' => Some(8),
            '九' => Some(9),
            _ => None,
        }
    };

    // Single digit
    if chars.len() == 1 {
        if chars[0] == '十' {
            return Some(10);
        }
        return digit(chars[0]);
    }

    // Two+ chars: parse as tens + units
    let mut result: u32 = 0;
    let mut i = 0;

    // Check for hundreds (百)
    if chars.len() >= 2 && chars.get(1) == Some(&'百') {
        if let Some(d) = digit(chars[0]) {
            result += d * 100;
            i = 2;
        }
    }

    // Parse tens
    if i < chars.len() {
        if chars[i] == '十' {
            // 十X = 10+X
            result += 10;
            i += 1;
        } else if i + 1 < chars.len() && chars[i + 1] == '十' {
            // N十 or N十X
            if let Some(d) = digit(chars[i]) {
                result += d * 10;
                i += 2; // skip past 十
            }
        } else if result == 0 {
            // Just a single digit like 五
            return digit(chars[i]);
        }
    }

    // Parse units
    if i < chars.len() {
        if let Some(d) = digit(chars[i]) {
            result += d;
        }
    }

    if result > 0 || s == "零" {
        Some(result)
    } else {
        None
    }
}

/// Map Chinese suffix 甲/乙/丙 to A/B/C
fn chinese_suffix_to_letter(c: char) -> Option<char> {
    match c {
        '甲' => Some('A'),
        '乙' => Some('B'),
        '丙' => Some('C'),
        _ => None,
    }
}

/// Translate a log item title algorithmically based on its kind.
/// Returns English version like "Lecture 21", "Homework 2", "Quiz 10A".
pub fn translate_title_algorithmic(kind: &str, title: &str) -> String {
    let en_kind = match kind {
        "Lecture" => "Lecture",
        "Discussion" => "Discussion",
        "Lab" => "Lab",
        "Homework" => "Homework",
        "Quiz" => "Quiz",
        "Midterm" => "Midterm",
        "Final" => "Final",
        "Project" => "Project",
        _ => "Other",
    };

    // Try pattern: 第X讲 (lecture-specific)
    if let Some(rest) = title.strip_prefix('第') {
        if let Some(num_str) = rest.strip_suffix('讲') {
            if let Some(n) = chinese_num_to_int(num_str) {
                return format!("{} {}", en_kind, n);
            }
        }
        // 第X次 pattern
        if let Some(num_str) = rest.strip_suffix('次') {
            if let Some(n) = chinese_num_to_int(num_str) {
                return format!("{} {}", en_kind, n);
            }
        }
    }

    // Try pattern: 期中考试X or 期末考试X
    if let Some(rest) = title.strip_prefix("期中考试") {
        if rest.is_empty() {
            return "Midterm".to_string();
        }
        if let Some(n) = chinese_num_to_int(rest) {
            return format!("Midterm {}", n);
        }
    }
    if let Some(rest) = title.strip_prefix("期末考试") {
        if rest.is_empty() {
            return "Final".to_string();
        }
        if let Some(n) = chinese_num_to_int(rest) {
            return format!("Final {}", n);
        }
    }

    // Try kind-prefixed patterns: 作业X, 测验X, 实验X, 讨论X, 讲座X
    let cn_kind_prefixes: &[(&str, &str)] = &[
        ("作业", "Homework"),
        ("测验", "Quiz"),
        ("实验", "Lab"),
        ("讨论", "Discussion"),
        ("讲座", "Lecture"),
        ("项目", "Project"),
    ];

    for (cn_prefix, en_name) in cn_kind_prefixes {
        if let Some(rest) = title.strip_prefix(cn_prefix) {
            if rest.is_empty() {
                return en_name.to_string();
            }
            // Check for suffix letter (甲/乙/丙)
            let last_char = rest.chars().last().unwrap();
            let (num_part, suffix) = if let Some(letter) = chinese_suffix_to_letter(last_char) {
                let num_str: String = rest.chars().take(rest.chars().count() - 1).collect();
                (num_str, Some(letter))
            } else {
                (rest.to_string(), None)
            };

            if let Some(n) = chinese_num_to_int(&num_part) {
                return match suffix {
                    Some(letter) => format!("{} {}{}", en_name, n, letter),
                    None => format!("{} {}", en_name, n),
                };
            }
        }
    }

    // Fallback: return original title
    title.to_string()
}

// ========== LLM Translation via OpenRouter ==========

/// Look up cached translations from the database.
/// Returns a vec of Option<String> in the same order as input texts.
pub async fn lookup_cached_translations(
    db: &mut Connection<Db>,
    texts: &[String],
) -> Vec<Option<String>> {
    let mut results = Vec::with_capacity(texts.len());
    for text in texts {
        let cached: Option<String> = sqlx::query_scalar(
            "SELECT translated_text FROM translations WHERE source_text = ? AND source_lang = 'zh' AND target_lang = 'en'"
        )
        .bind(text)
        .fetch_optional(&mut ***db)
        .await
        .unwrap_or(None);
        results.push(cached);
    }
    results
}

/// Translate a batch of texts using LLM (OpenRouter API).
/// Checks DB cache first, calls API for misses, stores results.
/// Returns translated texts in same order as input.
pub async fn translate_batch(
    db: &mut Connection<Db>,
    texts: &[String],
    course_context: &str,
) -> Vec<String> {
    if texts.is_empty() {
        return vec![];
    }

    // Deduplicate while preserving order
    let mut unique_texts: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for text in texts {
        if !text.is_empty() && seen.insert(text.clone()) {
            unique_texts.push(text.clone());
        }
    }

    // Check cache for all unique texts
    let mut cache_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut misses: Vec<String> = Vec::new();

    for text in &unique_texts {
        let cached: Option<String> = sqlx::query_scalar(
            "SELECT translated_text FROM translations WHERE source_text = ? AND source_lang = 'zh' AND target_lang = 'en'"
        )
        .bind(text)
        .fetch_optional(&mut ***db)
        .await
        .unwrap_or(None);

        if let Some(translation) = cached {
            cache_map.insert(text.clone(), translation);
        } else {
            misses.push(text.clone());
        }
    }

    // Call API for misses (retry up to 3 times)
    if !misses.is_empty() {
        let mut api_result = None;
        for _ in 0..3 {
            match call_openrouter_translate(&misses, course_context).await {
                Ok(translations) => {
                    api_result = Some(translations);
                    break;
                }
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
        if let Some(translations) = api_result {
            if translations.len() == misses.len() {
                for (source, translated) in misses.iter().zip(translations.iter()) {
                    // Store in DB cache
                    let _ = sqlx::query(
                        "INSERT OR REPLACE INTO translations (source_text, translated_text, source_lang, target_lang) VALUES (?, ?, 'zh', 'en')"
                    )
                    .bind(source)
                    .bind(translated)
                    .execute(&mut ***db)
                    .await;

                    cache_map.insert(source.clone(), translated.clone());
                }
            } else {
                // Mismatch in count — use originals as fallback
                for source in &misses {
                    cache_map.insert(source.clone(), source.clone());
                }
            }
        } else {
            // API failure — graceful degradation: use originals
            for source in &misses {
                cache_map.insert(source.clone(), source.clone());
            }
        }
    }

    // Map back to original order
    texts
        .iter()
        .map(|t| {
            if t.is_empty() {
                String::new()
            } else {
                cache_map.get(t).cloned().unwrap_or_else(|| t.clone())
            }
        })
        .collect()
}

/// Call the OpenRouter API to translate a batch of texts.
async fn call_openrouter_translate(
    texts: &[String],
    course_context: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")?;

    let numbered: String = texts
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Translate these Chinese items to English for a university course ({}). \
         These are topic descriptions and category names. \
         Return ONLY a JSON array of strings, with exactly {} elements, in the same order:\n{}",
        course_context,
        texts.len(),
        numbered
    );

    let client = reqwest::Client::new();
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "google/gemini-2.5-flash",
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("No content in response")?;

    // Strip markdown code fences if present
    let json_str = content
        .trim()
        .strip_prefix("```json")
        .or_else(|| content.trim().strip_prefix("```"))
        .unwrap_or(content.trim());
    let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

    let translations: Vec<String> = serde_json::from_str(json_str)?;
    Ok(translations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_num_to_int() {
        assert_eq!(chinese_num_to_int("一"), Some(1));
        assert_eq!(chinese_num_to_int("十"), Some(10));
        assert_eq!(chinese_num_to_int("十一"), Some(11));
        assert_eq!(chinese_num_to_int("二十"), Some(20));
        assert_eq!(chinese_num_to_int("二十一"), Some(21));
        assert_eq!(chinese_num_to_int("三十四"), Some(34));
        assert_eq!(chinese_num_to_int("零"), Some(0));
    }

    #[test]
    fn test_translate_title_algorithmic() {
        assert_eq!(translate_title_algorithmic("Lecture", "第二十一讲"), "Lecture 21");
        assert_eq!(translate_title_algorithmic("Homework", "作业二"), "Homework 2");
        assert_eq!(translate_title_algorithmic("Quiz", "测验十"), "Quiz 10");
        assert_eq!(translate_title_algorithmic("Midterm", "期中考试一"), "Midterm 1");
        assert_eq!(translate_title_algorithmic("Midterm", "期中考试"), "Midterm");
        assert_eq!(translate_title_algorithmic("Homework", "作业三甲"), "Homework 3A");
        // Fallback to original
        assert_eq!(translate_title_algorithmic("Other", "Something else"), "Something else");
    }
}
