//! 提问参数解析（纯逻辑，可单测）。支持一次多个问题（`-q`）。

/// 单个问题的原始参数（路径未解析/校验）。
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionArgs {
    pub message: String,
    pub options: Vec<String>,
    /// `-f`/`--file` 给出的原始路径（按出现顺序，未做解析/校验）。
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskArgs {
    /// 至少一个问题。
    pub questions: Vec<QuestionArgs>,
    /// 是否按 Markdown 渲染（全局，对所有问题生效）。
    pub is_markdown: bool,
}

/// 解析 `AskHuman <message> [-o <opt>] [-f <path>] [-q <message> ...] [--no-markdown]`。
///
/// 规则：
/// - 第一题可用位置参数或 `-q`；后续题必须 `-q`。
/// - `-o`/`-f` 归属「最近声明的问题」。
/// - `--no-markdown` 全局，可出现在任意位置。
///
/// 失败时返回中文错误描述。
pub fn parse_ask(args: &[String]) -> Result<AskArgs, String> {
    let mut questions: Vec<QuestionArgs> = Vec::new();
    let mut is_markdown = true;
    let mut seen_question_flag = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-q" | "--question" => {
                if i + 1 >= args.len() {
                    return Err(format!("{} 选项缺少参数值", arg));
                }
                questions.push(QuestionArgs {
                    message: args[i + 1].clone(),
                    options: Vec::new(),
                    files: Vec::new(),
                });
                seen_question_flag = true;
                i += 2;
            }
            "-o" | "--option" => {
                if i + 1 >= args.len() {
                    return Err(format!("{} 选项缺少参数值", arg));
                }
                let q = questions
                    .last_mut()
                    .ok_or_else(|| format!("{} 必须出现在某个问题之后", arg))?;
                q.options.push(args[i + 1].clone());
                i += 2;
            }
            "-f" | "--file" => {
                if i + 1 >= args.len() {
                    return Err(format!("{} 选项缺少参数值", arg));
                }
                let q = questions
                    .last_mut()
                    .ok_or_else(|| format!("{} 必须出现在某个问题之后", arg))?;
                q.files.push(args[i + 1].clone());
                i += 2;
            }
            "--no-markdown" => {
                is_markdown = false;
                i += 1;
            }
            a if a.starts_with('-') => {
                return Err(format!("未知选项: {}", a));
            }
            _ => {
                // 位置参数：仅允许作为第一个问题，且需在任何 -q 之前。
                if seen_question_flag || !questions.is_empty() {
                    return Err("位置参数只能作为第一个问题，且需在所有 -q 之前".to_string());
                }
                questions.push(QuestionArgs {
                    message: arg.clone(),
                    options: Vec::new(),
                    files: Vec::new(),
                });
                i += 1;
            }
        }
    }

    if questions.is_empty() {
        return Err("缺少提问内容".to_string());
    }

    Ok(AskArgs {
        questions,
        is_markdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn message_only() {
        let p = parse_ask(&v(&["hello"])).unwrap();
        assert_eq!(p.questions.len(), 1);
        assert_eq!(p.questions[0].message, "hello");
        assert!(p.questions[0].options.is_empty());
        assert!(p.is_markdown);
    }

    #[test]
    fn single_question_options_and_files() {
        let p = parse_ask(&v(&["msg", "-o", "A", "--option", "B", "-f", "a.md"])).unwrap();
        assert_eq!(p.questions.len(), 1);
        assert_eq!(p.questions[0].options, v(&["A", "B"]));
        assert_eq!(p.questions[0].files, v(&["a.md"]));
    }

    #[test]
    fn positional_first_then_q() {
        let p = parse_ask(&v(&["Q1", "-o", "A", "-q", "Q2", "-o", "B", "-f", "x.png"])).unwrap();
        assert_eq!(p.questions.len(), 2);
        assert_eq!(p.questions[0].message, "Q1");
        assert_eq!(p.questions[0].options, v(&["A"]));
        assert!(p.questions[0].files.is_empty());
        assert_eq!(p.questions[1].message, "Q2");
        assert_eq!(p.questions[1].options, v(&["B"]));
        assert_eq!(p.questions[1].files, v(&["x.png"]));
    }

    #[test]
    fn first_question_via_q_flag() {
        let p = parse_ask(&v(&["-q", "Q1", "-o", "A", "-q", "Q2"])).unwrap();
        assert_eq!(p.questions.len(), 2);
        assert_eq!(p.questions[0].message, "Q1");
        assert_eq!(p.questions[0].options, v(&["A"]));
        assert_eq!(p.questions[1].message, "Q2");
        assert!(p.questions[1].options.is_empty());
    }

    #[test]
    fn no_markdown_is_global() {
        let p = parse_ask(&v(&["Q1", "-q", "Q2", "--no-markdown"])).unwrap();
        assert!(!p.is_markdown);
        assert_eq!(p.questions.len(), 2);
    }

    #[test]
    fn option_before_any_question_errors() {
        assert!(parse_ask(&v(&["-o", "A"])).is_err());
        assert!(parse_ask(&v(&["-f", "a.md"])).is_err());
    }

    #[test]
    fn positional_after_q_errors() {
        assert!(parse_ask(&v(&["Q1", "-q", "Q2", "extra"])).is_err());
        assert!(parse_ask(&v(&["-q", "Q1", "Q2"])).is_err());
    }

    #[test]
    fn question_requires_value() {
        assert!(parse_ask(&v(&["-q"])).is_err());
        assert!(parse_ask(&v(&["Q1", "-q"])).is_err());
    }

    #[test]
    fn requires_at_least_one_question() {
        assert!(parse_ask(&v(&["--no-markdown"])).is_err());
        assert!(parse_ask(&v(&[])).is_err());
    }

    #[test]
    fn requires_option_value() {
        assert!(parse_ask(&v(&["msg", "-o"])).is_err());
    }

    #[test]
    fn requires_file_value() {
        assert!(parse_ask(&v(&["msg", "-f"])).is_err());
    }

    #[test]
    fn rejects_unknown_flag() {
        assert!(parse_ask(&v(&["msg", "--foo"])).is_err());
    }
}
