//! 抢答协调器：并行 Channel 的首个终态结果生效，其余被 `cancel_by_other` 收尾。

use crate::channels::Channel;
use crate::models::{AskRequest, ChannelResult};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

/// 拿到结果后如何退出进程。
#[derive(Clone)]
pub enum Exiter {
    /// GUI 模式：经 Tauri 事件循环退出（携带退出码）。
    Gui(AppHandle),
    /// headless 模式：直接退出进程。
    Process,
}

pub struct Coordinator {
    inner: Mutex<Inner>,
}

struct Inner {
    finished: bool,
    exiter: Exiter,
    request: AskRequest,
    channels: Vec<Arc<dyn Channel>>,
}

impl Coordinator {
    /// GUI 模式协调器。
    pub fn new(app: AppHandle, request: AskRequest) -> Arc<Self> {
        Self::with_exiter(Exiter::Gui(app), request)
    }

    /// headless 模式协调器（无 GUI，结果到达后直接退出进程）。
    pub fn new_headless(request: AskRequest) -> Arc<Self> {
        Self::with_exiter(Exiter::Process, request)
    }

    fn with_exiter(exiter: Exiter, request: AskRequest) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                finished: false,
                exiter,
                request,
                channels: Vec::new(),
            }),
        })
    }

    pub fn register(&self, channel: Arc<dyn Channel>) {
        self.inner.lock().unwrap().channels.push(channel);
    }

    /// 投递终态结果：仅首个生效；随后收尾其余 Channel，输出并退出进程。
    pub fn submit(&self, result: ChannelResult) {
        let (exiter, request_id, others) = {
            let mut inner = self.inner.lock().unwrap();
            if inner.finished {
                return;
            }
            inner.finished = true;
            let source = result.source_channel_id.clone();
            let others: Vec<Arc<dyn Channel>> = inner
                .channels
                .iter()
                .filter(|c| c.id() != source)
                .cloned()
                .collect();
            (inner.exiter.clone(), inner.request.id.clone(), others)
        };

        for ch in &others {
            ch.cancel_by_other();
        }

        let code = super::emit_result(&request_id, &result);
        let _ = std::io::stdout().flush();
        match exiter {
            Exiter::Gui(app) => app.exit(code),
            Exiter::Process => std::process::exit(code),
        }
    }
}
