//! 互动卡片高级版（A 方案）：按已发布模板组装 cardData（公有数据）与解析「提交」回调。
//!
//! 模板变量约定（见 `docs/plans/dingtalk-card-answers.md`）：
//! - 公有：`title`(标题) / `markdown`(正文) / `options`(对象数组,每项 `{text}`) /
//!   `submitted`(布尔) / `private_input`(字符串)。
//! - 提交按钮 `actionId="submit_action"`，回传 `params={user_input, selected_options}`。
//!
//! cardData 填充规则：复杂值（对象/数组）需转成 JSON 字符串放入 `cardParamMap`；
//! 布尔/数字同样以字符串下发（钉钉约定）。

use serde_json::{json, Value};

/// 「提交」按钮回传的 actionId。
pub const SUBMIT_ACTION_ID: &str = "submit_action";

/// 一次卡片「提交」回调的解析结果。
pub struct CardSubmit {
    pub user_id: String,
    pub out_track_id: String,
    /// 勾选的预定义选项（选项文本，已去重/过滤空串）。
    pub selected_options: Vec<String>,
    /// 补充文字输入（空则 None）。
    pub user_input: Option<String>,
}

/// 组装卡片公有数据 `cardParamMap`（值均为字符串）。
/// `title` 为题首；`markdown` 为问题正文；`options` 为预定义选项文本列表。
pub fn build_card_param_map(title: &str, markdown: &str, options: &[String]) -> Value {
    let option_objs: Vec<Value> = options.iter().map(|o| json!({ "text": o })).collect();
    json!({
        "title": title,
        "markdown": markdown,
        // 复杂类型 → JSON 字符串。
        "options": Value::Array(option_objs).to_string(),
        "submitted": "false",
        "private_input": "",
    })
}

/// 把一条卡片回调 `data` 解析为「提交」结果；非提交 / 非本类回调返回 None。
pub fn parse_card_submit(data: &Value) -> Option<CardSubmit> {
    let user_id = data
        .get("userId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let out_track_id = data
        .get("outTrackId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // content 优先，回退 value；二者皆为 JSON 字符串（也兼容已是对象的情况）。
    let inner: Value = data
        .get("content")
        .or_else(|| data.get("value"))
        .and_then(parse_maybe_json)?;
    let private = inner.get("cardPrivateData")?;

    // 必须是「提交」按钮触发。
    let is_submit = private
        .get("actionIds")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .any(|v| v.as_str() == Some(SUBMIT_ACTION_ID))
        })
        .unwrap_or(false);
    if !is_submit {
        return None;
    }

    let params = private.get("params");
    let selected_options = params
        .and_then(|p| p.get("selected_options"))
        .map(extract_strings)
        .unwrap_or_default();
    let user_input = params
        .and_then(|p| p.get("user_input"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(CardSubmit {
        user_id,
        out_track_id,
        selected_options,
        user_input,
    })
}

/// 把回调里可能是「JSON 字符串」或「对象」的字段统一解析成 `Value`。
fn parse_maybe_json(v: &Value) -> Option<Value> {
    match v {
        Value::String(s) => serde_json::from_str(s).ok(),
        other => Some(other.clone()),
    }
}

/// 从数组里抽取字符串列表：元素可为字符串或 `{text}`/`{value}` 对象；去重、过滤空串。
fn extract_strings(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(arr) = v.as_array() {
        for el in arr {
            let s = match el {
                Value::String(s) => s.clone(),
                Value::Object(_) => el
                    .get("text")
                    .or_else(|| el.get("value"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            };
            let s = s.trim().to_string();
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_param_map_stringifies_complex() {
        let m = build_card_param_map("Question 1/2", "要继续吗？", &["继续".into(), "停止".into()]);
        assert_eq!(m.get("title").unwrap(), "Question 1/2");
        assert_eq!(m.get("markdown").unwrap(), "要继续吗？");
        assert_eq!(m.get("submitted").unwrap(), "false");
        assert_eq!(m.get("private_input").unwrap(), "");
        // options 为 JSON 字符串
        let opts = m.get("options").unwrap().as_str().unwrap();
        let parsed: Value = serde_json::from_str(opts).unwrap();
        assert_eq!(parsed, json!([{ "text": "继续" }, { "text": "停止" }]));
    }

    #[test]
    fn parse_submit_text_array() {
        let data = json!({
            "userId": "u1",
            "outTrackId": "t1",
            "content": "{\"cardPrivateData\":{\"actionIds\":[\"submit_action\"],\"params\":{\"user_input\":\" hi \",\"selected_options\":[\"继续\",\"\",\"继续\"]}}}",
        });
        let s = parse_card_submit(&data).unwrap();
        assert_eq!(s.user_id, "u1");
        assert_eq!(s.out_track_id, "t1");
        assert_eq!(s.selected_options, vec!["继续".to_string()]);
        assert_eq!(s.user_input.as_deref(), Some("hi"));
    }

    #[test]
    fn parse_submit_object_array_and_empty_input() {
        let data = json!({
            "userId": "u1",
            "outTrackId": "t1",
            "value": {"cardPrivateData":{"actionIds":["submit_action"],"params":{"user_input":"","selected_options":[{"text":"A"},{"value":"B"}]}}},
        });
        let s = parse_card_submit(&data).unwrap();
        assert_eq!(s.selected_options, vec!["A".to_string(), "B".to_string()]);
        assert!(s.user_input.is_none());
    }

    #[test]
    fn parse_non_submit_returns_none() {
        let data = json!({
            "userId": "u1",
            "outTrackId": "t1",
            "content": "{\"cardPrivateData\":{\"actionIds\":[\"opt_0\"],\"params\":{}}}",
        });
        assert!(parse_card_submit(&data).is_none());
    }
}
