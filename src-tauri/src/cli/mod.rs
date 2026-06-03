pub mod args;
pub mod file_attachment;
pub mod help;
pub mod image_writer;
pub mod output;

use std::process::exit;

/// 入口分发：在创建任何窗口前按 argv 分流。
pub fn dispatch() {
    let argv: Vec<String> = std::env::args().collect();

    if argv.len() < 2 {
        eprintln!("错误: 缺少提问内容\n");
        println!("{}", help::help_text());
        exit(1);
    }

    match argv[1].as_str() {
        "--help" | "-h" => {
            println!("{}", help::help_text());
            exit(0);
        }
        "--version" | "-v" => {
            println!("{}", help::version_text());
            exit(0);
        }
        "--agent-help" => {
            println!("{}", help::agent_help_text());
            exit(0);
        }
        "--settings" => {
            crate::app::run_settings(crate::config::AppConfig::load());
        }
        // 第一题既可用位置参数，也可用 `-q`/`--question`；提问相关 flag 一律进入提问分支，
        // 由 `parse_ask` 给出精确错误（如缺少问题内容、选项需在问题之后）。
        first
            if first.starts_with('-')
                && !matches!(
                    first,
                    "-q" | "--question"
                        | "-o"
                        | "--option"
                        | "-f"
                        | "--file"
                        | "--no-markdown"
                ) =>
        {
            eprintln!("错误: 未知选项 {}\n", first);
            println!("{}", help::help_text());
            exit(1);
        }
        _ => match args::parse_ask(&argv[1..]) {
            Ok(parsed) => {
                let mut questions = Vec::with_capacity(parsed.questions.len());
                for q in parsed.questions {
                    let files = match file_attachment::resolve(&q.files) {
                        Ok(files) => files,
                        Err(e) => {
                            eprintln!("错误: {}", e);
                            exit(1);
                        }
                    };
                    questions.push(crate::models::Question::new(q.message, q.options, files));
                }
                let request = crate::models::AskRequest::new(questions, parsed.is_markdown);
                crate::app::run_ask(request, crate::config::AppConfig::load());
            }
            Err(e) => {
                eprintln!("错误: {}\n", e);
                println!("{}", help::help_text());
                exit(1);
            }
        },
    }
}
