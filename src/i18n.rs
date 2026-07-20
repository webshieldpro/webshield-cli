//! CLI localization (English/Russian).
//!
//! The language is resolved once at startup: `--lang` flag → env `WS_LANG` → system
//! locale (`LC_ALL`/`LC_MESSAGES`/`LANG`) → English by default. It is stored in a
//! global `OnceLock` and read via free functions, so the language does not have to be
//! threaded through every call. clap help is localized by mutating the command tree
//! (`localize_help`); runtime strings — via `tr`/`f`.

use std::env;
use std::sync::OnceLock;

use clap::builder::Command;
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Lang {
    /// English
    En,
    /// Russian (русский)
    Ru,
}

static LANG: OnceLock<Lang> = OnceLock::new();

pub fn set(lang: Lang) {
    let _ = LANG.set(lang);
}

pub fn get() -> Lang {
    *LANG.get().unwrap_or(&Lang::En)
}

fn parse(v: &str) -> Option<Lang> {
    match v.trim().to_lowercase().as_str() {
        "ru" | "rus" | "russian" | "русский" => Some(Lang::Ru),
        "en" | "eng" | "english" => Some(Lang::En),
        _ => None,
    }
}

/// Resolves the language from the flag/env/locale.
pub fn resolve(flag: Option<&str>) -> Lang {
    if let Some(l) = flag.and_then(parse) {
        return l;
    }
    if let Ok(v) = env::var("WS_LANG") {
        if let Some(l) = parse(&v) {
            return l;
        }
    }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = env::var(key) {
            let v = v.to_lowercase();
            if v.starts_with("ru") {
                return Lang::Ru;
            }
            if !v.is_empty() && v != "c" && v != "posix" {
                return Lang::En;
            }
        }
    }
    Lang::En
}

/// Pre-extracts the `--lang` value from raw arguments (before full parsing,
/// so that help prints in the right language straight away).
pub fn prescan_lang(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--lang=") {
            return Some(v.to_string());
        }
        if a == "--lang" {
            return it.next().cloned();
        }
    }
    None
}

/// Message keys. Each maps to an (English, Russian) pair.
#[derive(Clone, Copy)]
pub enum M {
    // Application and command groups (for help).
    AppAbout,
    CmdAuth,
    CmdDomains,
    CmdDns,
    CmdSites,
    CmdProxy,
    CmdStats,
    CmdBilling,
    CmdCompletion,
    // Common
    Yes,
    No,
    Dash,
    Empty,
    HValue,
    ErrorPrefix,
    ConfirmSuffix,
    ConfirmCancelled,
    // auth
    TokenPrompt,
    TokenWarnPrefix,
    LblProfile,
    LblApiUrl,
    LblToken,
    LblAccess,
    TokenSet,
    TokenUnset,
    LoginHint,
    TokenSavedOk,
    TokenSavedScoped,
    TokenSavedCode,
    TokenSavedProbeFail,
    TokenRemoved,
    ProfileNotFound,
    AccessOk,
    AccessInvalid,
    AccessForbidden,
    AccessUnexpected,
    NoToken,
    // domains
    HId,
    HDomain,
    HDelegated,
    HTariff,
    DomainCreated,
    DomainDeleted,
    DomainNotFound,
    ConfirmDeleteDomain,
    DelegationOk,
    DelegationNotDelegated,
    DelegationCurrentNs,
    DelegationMissingNs,
    DelegationExtraNs,
    DelegationNoNs,
    DelegationUnknown,
    DelegationPropagationNote,
    // dns
    HName,
    HType,
    HTtl,
    HProxy,
    HValues,
    DnsAdded,
    DnsSet,
    DnsRemoved,
    RecordNotFound,
    NothingToDelete,
    // sites
    HHost,
    HStatus,
    HVersion,
    HSize,
    HPath,
    HEtag,
    SiteCreated,
    SiteDisabled,
    PublishSummary,
    PublishNoChanges,
    PublishDryRun,
    Published,
    BucketPublishStarted,
    BucketPublished,
    BucketPublishFailed,
    BucketPublishTimeout,
    DeletedFiles,
    UploadBar,
    NotFoundSite,
    PublishNeedsSiteRef,
    NotADir,
    DirNotFound,
    // proxy
    HMode,
    HTarget,
    HSsl,
    HBotProt,
    ProxyCreated,
    ProxyUpdated,
    ProxyRemoved,
    ConfirmRemoveProxy,
    NotFoundProxy,
    // stats / billing
    HCurrency,
    HBalance,
    HMetric,
    HIp,
    HReason,
    HLastSeen,
    HRequests,
    NoBans,
    // client errors
    ErrNetwork,
    // ErrReadBody,
    ErrParse,
    ErrUnauthorized,
    ErrForbidden,
}

impl M {
    fn pair(self) -> (&'static str, &'static str) {
        use M::*;
        match self {
            AppAbout => ("WebShield command-line client", "Клиент командной строки WebShield"),
            CmdAuth => ("Authentication and profiles", "Аутентификация и профили"),
            CmdDomains => ("Domains (zones)", "Домены (зоны)"),
            CmdDns => ("DNS records", "DNS-записи домена"),
            CmdSites => ("Static sites and publishing", "Статические сайты и публикация"),
            CmdProxy => ("Proxy/redirect host edge settings", "Edge-настройки прокси/редиректов"),
            CmdStats => ("Statistics and protection", "Статистика и защита"),
            CmdBilling => ("Billing (balance, usage, tariffs)", "Биллинг (баланс, трафик, тарифы)"),
            CmdCompletion => ("Generate a shell completion script", "Скрипт автодополнения для оболочки"),

            Yes => ("yes", "да"),
            No => ("no", "нет"),
            Dash => ("—", "—"),
            Empty => ("— empty —", "— пусто —"),
            HValue => ("Value", "Значение"),
            ErrorPrefix => ("error:", "ошибка:"),
            ConfirmSuffix => ("[y/N]", "[y/N]"),
            ConfirmCancelled => ("cancelled by user", "отменено пользователем"),

            TokenPrompt => ("Token wsk_…: ", "Токен wsk_…: "),
            TokenWarnPrefix => (
                "warning: a token usually starts with `wsk_`.",
                "предупреждение: токен обычно начинается с `wsk_`.",
            ),
            LblProfile => ("Profile:", "Профиль: "),
            LblApiUrl => ("API URL:", "API URL: "),
            LblToken => ("Token:  ", "Токен:   "),
            LblAccess => ("Access: ", "Доступ:  "),
            TokenSet => ("set", "задан"),
            TokenUnset => ("not set", "не задан"),
            LoginHint => (
                "Run `webshield auth login` to store a token.",
                "Выполните `webshield auth login`, чтобы сохранить токен.",
            ),
            TokenSavedOk => (
                "token saved to profile `{profile}`, access confirmed.",
                "токен сохранён в профиль `{profile}`, доступ подтверждён.",
            ),
            TokenSavedScoped => (
                "token saved to profile `{profile}` (valid, but narrow scopes).",
                "токен сохранён в профиль `{profile}` (валиден, но со суженными скоупами).",
            ),
            TokenSavedCode => (
                "token saved, but the check returned HTTP {code}.",
                "токен сохранён, но проверка вернула HTTP {code}.",
            ),
            TokenSavedProbeFail => (
                "token saved, but the check failed: {err}",
                "токен сохранён, но проверка не удалась: {err}",
            ),
            TokenRemoved => (
                "token removed from profile `{profile}`.",
                "токен удалён из профиля `{profile}`.",
            ),
            ProfileNotFound => (
                "profile `{profile}` not found — nothing to remove.",
                "профиль `{profile}` не найден — нечего удалять.",
            ),
            AccessOk => ("API access confirmed", "доступ к API подтверждён"),
            AccessInvalid => ("token is invalid (401)", "токен недействителен (401)"),
            AccessForbidden => (
                "token valid, but no access to domains (403)",
                "токен валиден, но без доступа к доменам (403)",
            ),
            AccessUnexpected => ("unexpected response HTTP {code}", "неожиданный ответ HTTP {code}"),
            NoToken => (
                "no token for profile `{profile}`.\nRun `webshield auth login --token wsk_…` or set env WS_TOKEN.",
                "не найден токен для профиля `{profile}`.\nВыполните `webshield auth login --token wsk_…` или задайте env WS_TOKEN.",
            ),

            HId => ("ID", "ID"),
            HDomain => ("Domain", "Домен"),
            HDelegated => ("Delegated", "Делегирован"),
            HTariff => ("Tariff", "Тариф"),
            DomainCreated => (
                "domain `{name}` created (id {id}).",
                "домен `{name}` создан (id {id}).",
            ),
            DomainDeleted => ("domain `{name}` deleted.", "домен `{name}` удалён."),
            DomainNotFound => (
                "domain `{name}` not found among your domains",
                "домен `{name}` не найден среди ваших доменов",
            ),
            ConfirmDeleteDomain => (
                "Delete domain `{name}` and its zone? This is irreversible.",
                "Удалить домен `{name}` и его зону? Это необратимо.",
            ),
            DelegationOk => (
                "domain `{name}` is delegated to WebShield.",
                "домен `{name}` делегирован на WebShield.",
            ),
            DelegationNotDelegated => (
                "domain `{name}` is not delegated.",
                "домен `{name}` не делегирован.",
            ),
            DelegationCurrentNs => (
                "current NS at the parent zone: {ns}",
                "текущие NS в родительской зоне: {ns}",
            ),
            DelegationMissingNs => (
                "WebShield nameservers are missing at the registrar: {ns}",
                "у регистратора не указаны NS-серверы WebShield: {ns}",
            ),
            DelegationExtraNs => (
                "remove the other nameservers at the registrar: {ns}",
                "уберите у регистратора посторонние NS-серверы: {ns}",
            ),
            DelegationNoNs => (
                "no NS delegation found in the parent zone.",
                "NS-делегирование в родительской зоне не найдено.",
            ),
            DelegationUnknown => (
                "delegation status is unknown.",
                "статус делегирования неизвестен.",
            ),
            DelegationPropagationNote => (
                "nameserver changes at the registrar can take up to 48 hours to propagate.",
                "изменения NS у регистратора могут применяться до 48 часов.",
            ),

            HName => ("Name", "Имя"),
            HType => ("Type", "Тип"),
            HTtl => ("TTL", "TTL"),
            HProxy => ("Proxy", "Прокси"),
            HValues => ("Values", "Значения"),
            DnsAdded => (
                "{name} {type} {domain}: values added — {count}.",
                "{name} {type} {domain}: добавлено значений — {count}.",
            ),
            DnsSet => (
                "{name} {type} {domain}: values set — {count}.",
                "{name} {type} {domain}: установлено значений — {count}.",
            ),
            DnsRemoved => (
                "{name} {type} {domain}: values removed — {count}.",
                "{name} {type} {domain}: удалено значений — {count}.",
            ),
            RecordNotFound => ("record {name} {type} not found", "запись {name} {type} не найдена"),
            NothingToDelete => (
                "nothing to delete: record {name} {type} has no values",
                "нечего удалять: у записи {name} {type} нет значений",
            ),

            HHost => ("Host", "Хост"),
            HStatus => ("Status", "Статус"),
            HVersion => ("Version", "Версия"),
            HSize => ("Size", "Размер"),
            HPath => ("Path", "Путь"),
            HEtag => ("ETag", "ETag"),
            SiteCreated => (
                "site `{host}` created (id {id}).",
                "сайт `{host}` создан (id {id}).",
            ),
            SiteDisabled => (
                "site `{host}` unpublished.",
                "сайт `{host}` снят с публикации.",
            ),
            PublishSummary => (
                "Site #{id}: local {local}, server {server} — upload {up}, delete {del}, unchanged {same}.",
                "Сайт #{id}: локально {local}, на сервере {server} — залить {up}, удалить {del}, без изменений {same}.",
            ),
            PublishNoChanges => (
                "no changes — nothing to publish.",
                "изменений нет — публикация не требуется.",
            ),
            PublishDryRun => (
                "dry-run: no changes applied.",
                "dry-run: изменения не применяются.",
            ),
            Published => ("published.", "опубликовано."),
            BucketPublishStarted => (
                "publishing from bucket…",
                "публикация из бакета…",
            ),
            BucketPublished => (
                "published from bucket (version {version}).",
                "опубликовано из бакета (версия {version}).",
            ),
            BucketPublishFailed => (
                "bucket publish failed: {error}",
                "публикация из бакета не удалась: {error}",
            ),
            BucketPublishTimeout => (
                "bucket publish is still running; check the site status later.",
                "публикация из бакета ещё идёт; проверьте статус сайта позже.",
            ),
            DeletedFiles => ("files deleted: {count}", "удалено файлов: {count}"),
            UploadBar => (
                "  upload [{bar:30}] {pos}/{len} files",
                "  заливка [{bar:30}] {pos}/{len} файлов",
            ),
            NotFoundSite => ("static site `{host}` not found", "статический сайт `{host}` не найден"),
            PublishNeedsSiteRef => (
                "specify a site: pass a hostname or --site-id",
                "укажите сайт: передайте hostname или --site-id",
            ),
            NotADir => ("not a directory: {path}", "не каталог: {path}"),
            DirNotFound => ("directory not found: {path}", "каталог не найден: {path}"),

            HMode => ("Mode", "Режим"),
            HTarget => ("Target", "Цель"),
            HSsl => ("SSL", "SSL"),
            HBotProt => ("Bot protection", "Бот-защита"),
            ProxyCreated => (
                "proxy config for `{host}` created.",
                "конфиг прокси для `{host}` создан.",
            ),
            ProxyUpdated => (
                "proxy config for `{host}` updated.",
                "конфиг прокси для `{host}` обновлён.",
            ),
            ProxyRemoved => (
                "proxy config for `{host}` removed.",
                "конфиг прокси для `{host}` удалён.",
            ),
            ConfirmRemoveProxy => (
                "Remove edge config for `{host}`?",
                "Удалить edge-конфиг для `{host}`?",
            ),
            NotFoundProxy => (
                "proxy config for `{host}` not found",
                "конфиг прокси для `{host}` не найден",
            ),

            HCurrency => ("Currency", "Валюта"),
            HBalance => ("Balance", "Баланс"),
            HMetric => ("Metric", "Показатель"),
            HIp => ("IP", "IP"),
            HReason => ("Reason", "Причина"),
            HLastSeen => ("Last seen", "Последний запрос"),
            HRequests => ("Requests", "Запросов"),
            NoBans => ("no active bans", "активных банов нет"),

            ErrNetwork => ("network error while calling the API", "ошибка сети при запросе к API"),
            ErrReadBody => ("failed to read the response body", "не удалось прочитать тело ответа"),
            // ErrParse => ("failed to parse the API response", "не удалось разобрать ответ API"),
            // ErrUnauthorized => (
            //     "check the token (`webshield auth status`).",
            //     "Проверьте токен (`webshield auth status`).",
            // ),
            // ErrForbidden => (
            //     "the token may lack the required scope or be bound to another domain/site.",
            //     "Возможно, у токена нет нужного скоупа или он привязан к другому домену/сайту.",
            // ),
        }
    }
}

/// Localized fixed string.
pub fn tr(m: M) -> &'static str {
    let (en, ru) = m.pair();
    match get() {
        Lang::En => en,
        Lang::Ru => ru,
    }
}

/// Localized string with named `{key}` placeholder interpolation.
pub fn f(m: M, args: &[(&str, &str)]) -> String {
    let mut s = tr(m).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Localizes clap help: recursively sets command and argument descriptions
/// (including the built-in `help`/`version`) for the current language. clap's own
/// structural words (`Usage:`/`Commands:`/`Options:`/`[possible values]`/`[default]`/`[env]`)
/// are not localized — clap does not expose them for i18n; all descriptions are.
pub fn localize_help(mut cmd: Command) -> Command {
    if get() != Lang::Ru {
        return cmd; // the base (doc comments) is English
    }
    // Materialize the auto arguments (`help`/`version`) and the auto `help` subcommand,
    // otherwise the recursion will not see them — they are absent from the tree before build().
    cmd.build();
    localize_cmd(cmd, "")
}

fn localize_cmd(mut cmd: Command, parent: &str) -> Command {
    // Command path relative to the root: "", "auth", "dns add", …
    let name = cmd.get_name().to_string();
    let rel = if parent.is_empty() {
        String::new() // root (webshield)
    } else if parent == "\0" {
        name.clone()
    } else {
        format!("{parent} {name}")
    };

    if let Some(about) = cmd_about_ru(&rel) {
        cmd = cmd.about(about);
    }

    // Arguments (including the auto `help`/`version`).
    let arg_ids: Vec<String> = cmd
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    for id in arg_ids {
        if let Some(help) = arg_ru(&rel, &id) {
            // The built-in `help`/`version` have their own default long_help — override it
            // too, otherwise --help (long help) still shows the English text.
            let meta = id == "help" || id == "version";
            cmd = cmd.mut_arg(id, move |a| {
                let a = a.help(help);
                if meta {
                    a.long_help(help)
                } else {
                    a
                }
            });
        }
    }

    // Recurse into subcommands.
    let sub_names: Vec<String> = cmd
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    for sn in sub_names {
        // For children of the root the parent path = child name (the "\0" marker = root).
        let child_parent = if rel.is_empty() {
            "\0".to_string()
        } else {
            rel.clone()
        };
        cmd = cmd.mut_subcommand(&sn, move |c| localize_cmd(c, &child_parent));
    }
    cmd
}

/// Russian short description of a command by its path.
fn cmd_about_ru(rel: &str) -> Option<&'static str> {
    if rel == "help" || rel.ends_with(" help") {
        return Some("Показать справку по команде");
    }
    Some(match rel {
        "" => tr(M::AppAbout),
        "auth" => tr(M::CmdAuth),
        "auth login" => "Сохранить токен wsk_… в профиль и проверить его.",
        "auth status" => "Показать активный профиль и проверить доступ к API.",
        "auth logout" => "Удалить токен из активного профиля.",
        "domains" => tr(M::CmdDomains),
        "domains list" => "Список ваших доменов.",
        "domains add" => "Добавить домен (создать зону).",
        "domains get" => "Показать домен.",
        "domains remove" => "Удалить домен и его зону.",
        "domains check" => "Проверить делегирование (NS указывают на нас).",
        "dns" => tr(M::CmdDns),
        "dns list" => "Список DNS-записей домена.",
        "dns add" => "Добавить значение(я) к записи (для A/AAAA/TXT дополняет существующие).",
        "dns set" => "Заменить запись ровно на указанные значения.",
        "dns remove" => "Удалить значение(я) записи; без значений — удалить весь набор.",
        "dns dnssec" => "Управление DNSSEC.",
        "dns dnssec status" => "Статус DNSSEC и DS-записи для регистратора.",
        "dns dnssec enable" => "Включить онлайн-подписывание зоны.",
        "dns dnssec disable" => {
            "Выключить DNSSEC (заблокировано при живом DS у родителя; см. --force)."
        }
        "sites" => tr(M::CmdSites),
        "sites list" => "Список статических сайтов.",
        "sites create" => "Создать статический сайт на хосте.",
        "sites publish" => "Инкрементально опубликовать каталог как содержимое сайта.",
        "sites files" => "Список файлов текущего draft.",
        "sites disable" => "Снять сайт с публикации.",
        "proxy" => tr(M::CmdProxy),
        "proxy list" => "Список конфигов прокси/редирект-хостов.",
        "proxy get" => "Показать конфиг хоста.",
        "proxy set" => "Создать или обновить конфиг хоста (частичное обновление, если существует).",
        "proxy remove" => "Удалить конфиг хоста.",
        "stats" => tr(M::CmdStats),
        "stats summary" => "Сводка по трафику/запросам домена.",
        "stats bans" => "Активные баны/челленджи домена.",
        "billing" => tr(M::CmdBilling),
        "billing balance" => "Баланс счёта по валютам.",
        "billing usage" => "Потребление трафика домена относительно лимита тарифа.",
        "billing tariffs" => "Текущий и доступные тарифы домена.",
        "completion" => tr(M::CmdCompletion),
        _ => return None,
    })
}

/// Russian description of an argument by command path and argument id.
fn arg_ru(rel: &str, id: &str) -> Option<&'static str> {
    // Context-dependent.
    match (rel, id) {
        ("auth login", "token") => {
            return Some("Токен wsk_… (спросим интерактивно, если не задан)")
        }
        ("auth login", "api_url") => return Some("Базовый URL API для профиля"),
        ("sites create", "domain") => return Some("Домен-владелец"),
        ("proxy set", "domain") => return Some("Домен-владелец (обязателен при создании)"),
        ("dns remove", "value") => {
            return Some("Конкретные значения для удаления (иначе удаляется весь набор)")
        }
        _ => {}
    }
    if rel.starts_with("domains ") && id == "name" {
        return Some("Имя домена");
    }
    if rel.starts_with("dns ") && id == "name" {
        return Some("Имя записи относительно домена или `@` для апекса");
    }
    // Shared, by id.
    Some(match id {
        "help" => "Показать справку",
        "version" => "Показать версию",
        "profile" => "Профиль конфигурации (по умолчанию активный из config.toml)",
        "api_url" => "Базовый URL API (переопределяет профиль)",
        "token" => "Персональный токен `wsk_…` (переопределяет профиль)",
        "lang" => "Язык интерфейса (en/ru); по умолчанию WS_LANG или системная локаль",
        "output" => "Формат вывода",
        "yes" => "Не спрашивать подтверждения для разрушительных операций",
        "domain" => "Домен",
        "import" => "Импорт записей при создании: scan | none",
        "rr_type" => "Фильтр по типу записи (A, AAAA, CNAME, …)",
        "TYPE" => "Тип записи",
        "value" => "Одно или несколько значений",
        "ttl" => "TTL записи (секунды)",
        "force" => "Снять подпись даже при живом DS у родителя (риск SERVFAIL)",
        "hostname" => "Hostname хоста",
        "dir" => "Каталог собранного сайта",
        "dry_run" => "Показать план без изменений",
        "range" => "Период, например 24h, 7d",
        "mode" => "Режим: proxy | redirect",
        "redirect_target" => "Целевой hostname редиректа (для mode=redirect)",
        "ssl" => "Требовать HTTPS (true/false)",
        "bot_protection" => "Бот-защита (true/false)",
        "captcha" => "Проверка капчей (true/false)",
        "http2" => "HTTP/2 (true/false)",
        "http3" => "HTTP/3 (true/false)",
        "max_body_mb" => "Максимальный размер тела запроса, МБ (0 = без лимита)",
        "block_bots" => "Слаги блокируемых ботов через запятую (см. `webshield bots`)",
        "shell" => "Оболочка: bash, zsh, fish, powershell, elvish, nushell",
        _ => return None,
    })
}
