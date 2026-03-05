use std::collections::HashMap;
use std::sync::LazyLock;

static TRANSLATIONS: LazyLock<HashMap<&'static str, HashMap<&'static str, &'static str>>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        let mut en = HashMap::new();
        en.insert("app.name", "MiniBot MVP");
        en.insert("app.starting_agent", "Starting MiniBot Agent...");
        en.insert("app.starting_gateway", "Starting Gateway at {}:{}");
        en.insert("app.version", "MiniBot MVP v{}");
        en.insert("app.config_dir", "Configuration directory");
        en.insert(
            "agent.started",
            "MiniBot Agent started. Type 'exit' to quit.",
        );
        en.insert("agent.exit_hint", "Type 'exit' to quit.");
        en.insert("agent.response", "{}");
        en.insert(
            "agent.max_time_exceeded",
            "Max execution time ({}) exceeded",
        );
        en.insert(
            "agent.max_iterations_exceeded",
            "Max tool iterations ({}) exceeded",
        );
        en.insert("error.prefix", "Error: {}");
        en.insert("error.provider_init", "Failed to initialize provider: {}");
        en.insert("error.agent_run", "Agent run failed: {}");
        en.insert("error.memory_init", "Failed to initialize memory: {}");
        en.insert("error.config_load", "Failed to load configuration: {}");
        en.insert("gateway.listening", "Gateway server listening on {}");
        en.insert("gateway.webhook_received", "Received webhook request: {}");
        en.insert("gateway.agent_response", "Agent response: {}");
        en.insert("gateway.agent_error", "Agent error: {}");
        m.insert("en", en);

        let mut zh_tw = HashMap::new();
        zh_tw.insert("app.name", "MiniBot MVP");
        zh_tw.insert("app.starting_agent", "正在啟動 MiniBot Agent...");
        zh_tw.insert("app.starting_gateway", "正在啟動閘道器 {}:{}");
        zh_tw.insert("app.version", "MiniBot MVP 第 {} 版");
        zh_tw.insert("app.config_dir", "配置目錄");
        zh_tw.insert("agent.started", "MiniBot Agent 已啟動。輸入 'exit' 結束。");
        zh_tw.insert("agent.exit_hint", "輸入 'exit' 結束。");
        zh_tw.insert("agent.response", "{}");
        zh_tw.insert("agent.max_time_exceeded", "已超過最大執行時間 ({}) 秒");
        zh_tw.insert(
            "agent.max_iterations_exceeded",
            "已超過最大工具迭代次數 ({})",
        );
        zh_tw.insert("error.prefix", "錯誤：{}");
        zh_tw.insert("error.provider_init", "供應商初始化失敗：{}");
        zh_tw.insert("error.agent_run", "Agent 執行失敗：{}");
        zh_tw.insert("error.memory_init", "記憶體初始化失敗：{}");
        zh_tw.insert("error.config_load", "載入配置失敗：{}");
        zh_tw.insert("gateway.listening", "閘道器服務正在監聽 {}");
        zh_tw.insert("gateway.webhook_received", "收到 Webhook 請求：{}");
        zh_tw.insert("gateway.agent_response", "Agent 回應：{}");
        zh_tw.insert("gateway.agent_error", "Agent 錯誤：{}");
        m.insert("zh-tw", zh_tw);

        let mut zh_cn = HashMap::new();
        zh_cn.insert("app.name", "MiniBot MVP");
        zh_cn.insert("app.starting_agent", "正在启动 MiniBot Agent...");
        zh_cn.insert("app.starting_gateway", "正在启动网关 {}:{}");
        zh_cn.insert("app.version", "MiniBot MVP 第 {} 版");
        zh_cn.insert("app.config_dir", "配置目录");
        zh_cn.insert("agent.started", "MiniBot Agent 已启动。输入 'exit' 结束。");
        zh_cn.insert("agent.exit_hint", "输入 'exit' 结束。");
        zh_cn.insert("agent.response", "{}");
        zh_cn.insert("agent.max_time_exceeded", "已超过最大执行时间 ({}) 秒");
        zh_cn.insert(
            "agent.max_iterations_exceeded",
            "已超过最大工具迭代次数 ({})",
        );
        zh_cn.insert("error.prefix", "错误：{}");
        zh_cn.insert("error.provider_init", "供应商初始化失败：{}");
        zh_cn.insert("error.agent_run", "Agent 执行失败：{}");
        zh_cn.insert("error.memory_init", "记忆体初始化失败：{}");
        zh_cn.insert("error.config_load", "加载配置失败：{}");
        zh_cn.insert("gateway.listening", "网关服务正在监听 {}");
        zh_cn.insert("gateway.webhook_received", "收到 Webhook 请求：{}");
        zh_cn.insert("gateway.agent_response", "Agent 回应：{}");
        zh_cn.insert("gateway.agent_error", "Agent 错误：{}");
        m.insert("zh-cn", zh_cn);

        m
    });

static CURRENT_LOCALE: LazyLock<Locale> = LazyLock::new(|| {
    std::env::var("MINIBOT_LOCALE")
        .ok()
        .map(|s| Locale::from_str(&s))
        .unwrap_or_default()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    ZhTw,
    ZhCn,
}

impl Locale {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "zh-tw" | "zh-hant" | "zh-hant-tw" => Locale::ZhTw,
            "zh-cn" | "zh-hans" | "zh" => Locale::ZhCn,
            _ => Locale::En,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::ZhTw => "zh-tw",
            Locale::ZhCn => "zh-cn",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct I18n {
    locale: Locale,
}

impl I18n {
    pub fn new(locale: Locale) -> Self {
        Self { locale }
    }

    pub fn from_env() -> Self {
        let locale = std::env::var("MINIBOT_LOCALE")
            .ok()
            .map(|s| Locale::from_str(&s))
            .unwrap_or_default();
        Self { locale }
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    pub fn t(&self, key: &str) -> String {
        let locale_str = self.locale.as_str();
        TRANSLATIONS
            .get(locale_str)
            .and_then(|m| m.get(key))
            .map(|s| s.to_string())
            .unwrap_or_else(|| key.to_string())
    }

    pub fn t_with_args(&self, key: &str, args: &[&dyn std::fmt::Display]) -> String {
        let template = self.t(key);
        if args.is_empty() {
            template
        } else {
            args.iter().fold(template, |acc, arg| {
                if let Some(pos) = acc.find("{}") {
                    let mut result = acc.clone();
                    result.replace_range(pos..pos + 2, &arg.to_string());
                    result
                } else {
                    acc
                }
            })
        }
    }
}

pub fn tr(key: &str) -> String {
    let locale_str = CURRENT_LOCALE.as_str();
    TRANSLATIONS
        .get(locale_str)
        .and_then(|m| m.get(key))
        .map(|s| s.to_string())
        .unwrap_or_else(|| key.to_string())
}

pub fn tr_with_args(key: &str, args: &[&dyn std::fmt::Display]) -> String {
    let template = tr(key);
    if args.is_empty() {
        template.to_string()
    } else {
        args.iter().fold(template.to_string(), |acc, arg| {
            if let Some(pos) = acc.find("{}") {
                let mut result = acc.clone();
                result.replace_range(pos..pos + 2, &arg.to_string());
                result
            } else {
                acc
            }
        })
    }
}

#[macro_export]
macro_rules! t {
    ($i18n:expr, $key:expr) => {
        $i18n.t($key)
    };
    ($i18n:expr, $key:expr, $($arg:expr),*) => {{
        let args: Vec<&dyn std::fmt::Display> = vec![$(&$arg as &dyn std::fmt::Display),*];
        $i18n.t_with_args($key, &args)
    }};
}

pub use t as tr;
