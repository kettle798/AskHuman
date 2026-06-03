//! 结果区块格式化（与当前 Swift 版逐字对齐）。

pub const CANCEL_STATUS_TEXT: &str =
    "用户取消了操作，你必须重新询问用户是否确定要取消，直到用户给出明确答复";

/// 某题未作答时的状态文案。
pub const UNANSWERED_STATUS_TEXT: &str = "用户未回答此问题";

/// 取消路径输出。
pub fn cancel_output() -> String {
    format!("[状态]\n{}", CANCEL_STATUS_TEXT)
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

    fn body(&self) -> String {
        send_output(
            self.selected_options,
            self.user_input,
            self.image_paths,
            self.file_paths,
        )
    }
}

fn unanswered_output() -> String {
    format!("[状态]\n{}", UNANSWERED_STATUS_TEXT)
}

/// 按问题聚合「发送」路径的输出（取消路径请直接用 `cancel_output`）。
///
/// - 单题：现状格式（无 `# Q1` 头）；空回答 → `用户未回答此问题`。
/// - 多题：每题 `# Qn` + 区块，题间用 `---` 分隔；未答题为 `用户未回答此问题`；
///   全部未答 → 仅输出一次取消提示。
pub fn aggregate_output(answers: &[RenderedAnswer]) -> String {
    if answers.len() <= 1 {
        return match answers.first() {
            Some(a) if !a.is_empty() => a.body(),
            _ => unanswered_output(),
        };
    }

    if answers.iter().all(|a| a.is_empty()) {
        return cancel_output();
    }

    answers
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let body = if a.is_empty() {
                unanswered_output()
            } else {
                a.body()
            };
            format!("# Q{}\n{}", i + 1, body)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// 成功路径输出（图片已落盘，传入路径列表；文件为用户拖入的非图片绝对路径，直接透传）。
pub fn send_output(
    selected_options: &[String],
    user_input: Option<&str>,
    image_paths: &[String],
    file_paths: &[String],
) -> String {
    let mut sections: Vec<String> = Vec::new();

    if !selected_options.is_empty() {
        sections.push(format!("[选择的选项]\n{}", selected_options.join(", ")));
    }

    if let Some(input) = user_input {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            sections.push(format!("[用户输入]\n{}", trimmed));
        }
    }

    if !image_paths.is_empty() {
        sections.push(format!("[图片]\n{}", image_paths.join("\n")));
    }

    if !file_paths.is_empty() {
        sections.push(format!("[文件]\n{}", file_paths.join("\n")));
    }

    if sections.is_empty() {
        sections.push("[用户输入]\n用户确认继续".to_string());
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn options_only() {
        let out = send_output(&s(&["A", "B"]), None, &[], &[]);
        assert_eq!(out, "[选择的选项]\nA, B");
    }

    #[test]
    fn input_trimmed() {
        let out = send_output(&[], Some("  你好  \n"), &[], &[]);
        assert_eq!(out, "[用户输入]\n你好");
    }

    #[test]
    fn empty_input_omitted() {
        let out = send_output(&[], Some("   "), &[], &[]);
        assert_eq!(out, "[用户输入]\n用户确认继续");
    }

    #[test]
    fn all_sections_blank_line_separated() {
        let out = send_output(&s(&["A"]), Some("hi"), &s(&["/tmp/a.png"]), &[]);
        assert_eq!(out, "[选择的选项]\nA\n\n[用户输入]\nhi\n\n[图片]\n/tmp/a.png");
    }

    #[test]
    fn files_section_after_images() {
        let out = send_output(&[], Some("hi"), &s(&["/tmp/a.png"]), &s(&["/tmp/b.md"]));
        assert_eq!(out, "[用户输入]\nhi\n\n[图片]\n/tmp/a.png\n\n[文件]\n/tmp/b.md");
    }

    #[test]
    fn empty_all_confirms_continue() {
        let out = send_output(&[], None, &[], &[]);
        assert_eq!(out, "[用户输入]\n用户确认继续");
    }

    #[test]
    fn cancel_text() {
        assert_eq!(
            cancel_output(),
            "[状态]\n用户取消了操作，你必须重新询问用户是否确定要取消，直到用户给出明确答复"
        );
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
        let out = aggregate_output(&[ans(&opts, Some("hi"), &[], &[])]);
        assert_eq!(out, "[选择的选项]\nA\n\n[用户输入]\nhi");
    }

    #[test]
    fn single_empty_is_unanswered() {
        let out = aggregate_output(&[ans(&[], Some("   "), &[], &[])]);
        assert_eq!(out, "[状态]\n用户未回答此问题");
    }

    #[test]
    fn multi_all_answered_grouped() {
        let o1 = s(&["A"]);
        let out = aggregate_output(&[
            ans(&o1, None, &[], &[]),
            ans(&[], Some("好的"), &[], &[]),
        ]);
        assert_eq!(
            out,
            "# Q1\n[选择的选项]\nA\n\n---\n\n# Q2\n[用户输入]\n好的"
        );
    }

    #[test]
    fn multi_partial_unanswered() {
        let o1 = s(&["A"]);
        let out = aggregate_output(&[ans(&o1, None, &[], &[]), ans(&[], None, &[], &[])]);
        assert_eq!(out, "# Q1\n[选择的选项]\nA\n\n---\n\n# Q2\n[状态]\n用户未回答此问题");
    }

    #[test]
    fn multi_all_unanswered_is_cancel() {
        let out = aggregate_output(&[ans(&[], None, &[], &[]), ans(&[], Some(" "), &[], &[])]);
        assert_eq!(out, cancel_output());
    }
}
