//! 结果区块格式化（区块结构固定，文案随界面语言本地化）。
//!
//! 区块标记（`[选择的选项]` 等）、状态文案取自 `i18n::tr`，与 `--agent-help` 共用同一套 key，
//! 保证「AI 看到的实际输出」与「agent-help 文档」一致。结构（分组顺序/`# Qn`/`---`）不随语言变化。

use crate::i18n::{tr, Lang};

/// 取消路径输出。
pub fn cancel_output(lang: Lang) -> String {
    format!("{}\n{}", tr(lang, "marker.status"), tr(lang, "status.cancel"))
}

/// 单题的已渲染回答（图片已落盘为路径，文件为透传的绝对路径）。
pub struct RenderedAnswer<'a> {
    pub selected_options: &'a [String],
    pub user_input: Option<&'a str>,
    pub image_paths: &'a [String],
    pub file_paths: &'a [String],
}

impl RenderedAnswer<'_> {
    /// 空回答：没选项、没（去空白后的）输入、没图片、没回复文件。
    fn is_empty(&self) -> bool {
        self.selected_options.is_empty()
            && self
                .user_input
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            && self.image_paths.is_empty()
            && self.file_paths.is_empty()
    }

    fn body(&self, lang: Lang) -> String {
        send_output(
            lang,
            self.selected_options,
            self.user_input,
            self.image_paths,
            self.file_paths,
        )
    }
}

fn unanswered_output(lang: Lang) -> String {
    format!("{}\n{}", tr(lang, "marker.status"), tr(lang, "status.unanswered"))
}

/// 按问题聚合「发送」路径的输出（取消路径请直接用 `cancel_output`）。
///
/// - 单题：现状格式（无 `# Q1` 头）；空回答 → 未作答状态。
/// - 多题：每题 `# Qn` + 区块，题间用 `---` 分隔；未答题为未作答状态；
///   全部未答 → 仅输出一次取消提示。
pub fn aggregate_output(lang: Lang, answers: &[RenderedAnswer]) -> String {
    if answers.len() <= 1 {
        return match answers.first() {
            Some(a) if !a.is_empty() => a.body(lang),
            _ => unanswered_output(lang),
        };
    }

    if answers.iter().all(|a| a.is_empty()) {
        return cancel_output(lang);
    }

    answers
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let body = if a.is_empty() {
                unanswered_output(lang)
            } else {
                a.body(lang)
            };
            format!("# Q{}\n{}", i + 1, body)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// 成功路径输出（图片已落盘，传入路径列表；文件为用户拖入的非图片绝对路径，直接透传）。
pub fn send_output(
    lang: Lang,
    selected_options: &[String],
    user_input: Option<&str>,
    image_paths: &[String],
    file_paths: &[String],
) -> String {
    let mut sections: Vec<String> = Vec::new();

    if !selected_options.is_empty() {
        sections.push(format!("{}\n{}", tr(lang, "marker.options"), selected_options.join(", ")));
    }

    if let Some(input) = user_input {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            sections.push(format!("{}\n{}", tr(lang, "marker.input"), trimmed));
        }
    }

    if !image_paths.is_empty() {
        sections.push(format!("{}\n{}", tr(lang, "marker.images"), image_paths.join("\n")));
    }

    if !file_paths.is_empty() {
        sections.push(format!("{}\n{}", tr(lang, "marker.files"), file_paths.join("\n")));
    }

    if sections.is_empty() {
        sections.push(format!("{}\n{}", tr(lang, "marker.input"), tr(lang, "status.confirmContinue")));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    // 结构断言用英文（源语言）；中文仅抽样验证标记会随语言变化。

    #[test]
    fn options_only() {
        let out = send_output(Lang::En, &s(&["A", "B"]), None, &[], &[]);
        assert_eq!(out, "[Selected options]\nA, B");
    }

    #[test]
    fn input_trimmed() {
        let out = send_output(Lang::En, &[], Some("  hi  \n"), &[], &[]);
        assert_eq!(out, "[User input]\nhi");
    }

    #[test]
    fn empty_input_omitted() {
        let out = send_output(Lang::En, &[], Some("   "), &[], &[]);
        assert_eq!(out, "[User input]\nUser confirmed to continue");
    }

    #[test]
    fn all_sections_blank_line_separated() {
        let out = send_output(Lang::En, &s(&["A"]), Some("hi"), &s(&["/tmp/a.png"]), &[]);
        assert_eq!(out, "[Selected options]\nA\n\n[User input]\nhi\n\n[Images]\n/tmp/a.png");
    }

    #[test]
    fn files_section_after_images() {
        let out = send_output(Lang::En, &[], Some("hi"), &s(&["/tmp/a.png"]), &s(&["/tmp/b.md"]));
        assert_eq!(out, "[User input]\nhi\n\n[Images]\n/tmp/a.png\n\n[Files]\n/tmp/b.md");
    }

    #[test]
    fn empty_all_confirms_continue() {
        let out = send_output(Lang::En, &[], None, &[], &[]);
        assert_eq!(out, "[User input]\nUser confirmed to continue");
    }

    #[test]
    fn markers_localized_to_zh() {
        let out = send_output(Lang::Zh, &s(&["A"]), Some("你好"), &[], &[]);
        assert_eq!(out, "[选择的选项]\nA\n\n[用户输入]\n你好");
    }

    #[test]
    fn cancel_text() {
        assert!(cancel_output(Lang::En).starts_with("[Status]\n"));
        assert!(cancel_output(Lang::Zh).starts_with("[状态]\n"));
    }

    fn ans<'a>(
        opts: &'a [String],
        input: Option<&'a str>,
        imgs: &'a [String],
        files: &'a [String],
    ) -> RenderedAnswer<'a> {
        RenderedAnswer {
            selected_options: opts,
            user_input: input,
            image_paths: imgs,
            file_paths: files,
        }
    }

    #[test]
    fn single_answered_keeps_current_format() {
        let opts = s(&["A"]);
        let out = aggregate_output(Lang::En, &[ans(&opts, Some("hi"), &[], &[])]);
        assert_eq!(out, "[Selected options]\nA\n\n[User input]\nhi");
    }

    #[test]
    fn single_empty_is_unanswered() {
        let out = aggregate_output(Lang::En, &[ans(&[], Some("   "), &[], &[])]);
        assert_eq!(out, "[Status]\nThe user did not answer this question");
    }

    #[test]
    fn multi_all_answered_grouped() {
        let o1 = s(&["A"]);
        let out = aggregate_output(
            Lang::En,
            &[ans(&o1, None, &[], &[]), ans(&[], Some("ok"), &[], &[])],
        );
        assert_eq!(out, "# Q1\n[Selected options]\nA\n\n---\n\n# Q2\n[User input]\nok");
    }

    #[test]
    fn multi_partial_unanswered() {
        let o1 = s(&["A"]);
        let out = aggregate_output(
            Lang::En,
            &[ans(&o1, None, &[], &[]), ans(&[], None, &[], &[])],
        );
        assert_eq!(
            out,
            "# Q1\n[Selected options]\nA\n\n---\n\n# Q2\n[Status]\nThe user did not answer this question"
        );
    }

    #[test]
    fn multi_all_unanswered_is_cancel() {
        let out =
            aggregate_output(Lang::En, &[ans(&[], None, &[], &[]), ans(&[], Some(" "), &[], &[])]);
        assert_eq!(out, cancel_output(Lang::En));
    }
}
