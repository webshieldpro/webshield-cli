//! The set of strings every locale must provide.
//!
//! Fixed strings are `&'static str`, templates are function pointers — the locale
//! files (`locales/*.iro`) supply closures, so a wrong number of placeholders does
//! not compile.

#[derive(Debug)]
pub struct Locale {
    // Help: application and command groups.
    pub app_about: &'static str,

    pub cmd_auth: &'static str,
    pub cmd_domains: &'static str,
    pub cmd_dns: &'static str,
    pub cmd_sites: &'static str,
    pub cmd_proxy: &'static str,
    pub cmd_stats: &'static str,
    pub cmd_billing: &'static str,
    pub cmd_completion: &'static str,

    // Help: global arguments.
    pub arg_profile: &'static str,
    pub arg_api_url: &'static str,
    pub arg_token: &'static str,
    pub arg_output: &'static str,
    pub arg_output_table: &'static str,
    pub arg_output_json: &'static str,
    pub arg_yes: &'static str,
    pub arg_page: &'static str,
    pub arg_shell: &'static str,
    pub arg_domain: &'static str,
    pub arg_hostname: &'static str,

    // Help: auth.
    pub cmd_auth_login: &'static str,
    pub cmd_auth_status: &'static str,
    pub cmd_auth_logout: &'static str,
    pub arg_login_token: &'static str,
    pub arg_login_api_url: &'static str,

    // Help: domains.
    pub cmd_domains_list: &'static str,
    pub cmd_domains_add: &'static str,
    pub cmd_domains_get: &'static str,
    pub cmd_domains_remove: &'static str,
    pub cmd_domains_check: &'static str,
    pub arg_domain_name: &'static str,
    pub arg_domains_import: &'static str,

    // Help: dns.
    pub cmd_dns_list: &'static str,
    pub cmd_dns_add: &'static str,
    pub cmd_dns_set: &'static str,
    pub cmd_dns_remove: &'static str,
    pub cmd_dns_dnssec: &'static str,
    pub cmd_dnssec_status: &'static str,
    pub cmd_dnssec_enable: &'static str,
    pub cmd_dnssec_disable: &'static str,
    pub arg_record_name: &'static str,
    pub arg_record_type: &'static str,
    pub arg_record_type_filter: &'static str,
    pub arg_record_value: &'static str,
    pub arg_record_value_remove: &'static str,
    pub arg_record_ttl: &'static str,
    pub arg_dnssec_force: &'static str,

    // Help: sites.
    pub cmd_sites_list: &'static str,
    pub cmd_sites_create: &'static str,
    pub cmd_sites_publish: &'static str,
    pub cmd_sites_publish_bucket: &'static str,
    pub cmd_sites_files: &'static str,
    pub cmd_sites_disable: &'static str,
    pub arg_site_hostname: &'static str,
    pub arg_site_domain: &'static str,
    pub arg_publish_hostname: &'static str,
    pub arg_publish_site_id: &'static str,
    pub arg_publish_dir: &'static str,
    pub arg_publish_dry_run: &'static str,
    pub arg_bucket: &'static str,
    pub arg_bucket_path: &'static str,

    // Help: proxy.
    pub cmd_proxy_list: &'static str,
    pub cmd_proxy_get: &'static str,
    pub cmd_proxy_set: &'static str,
    pub cmd_proxy_remove: &'static str,
    pub arg_proxy_domain: &'static str,
    pub arg_proxy_mode: &'static str,
    pub arg_proxy_redirect_target: &'static str,
    pub arg_proxy_ssl: &'static str,
    pub arg_proxy_bot_protection: &'static str,
    pub arg_proxy_captcha: &'static str,
    pub arg_proxy_http2: &'static str,
    pub arg_proxy_http3: &'static str,
    pub arg_proxy_max_body: &'static str,
    pub arg_proxy_block_bots: &'static str,

    // Help: stats and billing.
    pub cmd_stats_summary: &'static str,
    pub cmd_stats_bans: &'static str,
    pub arg_range: &'static str,
    pub cmd_billing_balance: &'static str,
    pub cmd_billing_usage: &'static str,
    pub cmd_billing_tariffs: &'static str,

    // Common.
    pub yes: &'static str,
    pub no: &'static str,
    pub dash: &'static str,
    pub empty: &'static str,
    pub h_value: &'static str,
    pub error_prefix: &'static str,
    pub confirm_suffix: &'static str,
    pub confirm_cancelled: &'static str,

    // auth.
    pub token_prompt: &'static str,
    pub token_warn_prefix: &'static str,
    pub token_empty: &'static str,
    pub lbl_profile: &'static str,
    pub lbl_api_url: &'static str,
    pub lbl_token: &'static str,
    pub lbl_lang: &'static str,
    pub lbl_access: &'static str,
    pub token_set: &'static str,
    pub token_unset: &'static str,
    pub login_hint: &'static str,
    pub token_saved_ok: fn(profile: &str) -> String,
    pub token_saved_scoped: fn(profile: &str) -> String,
    pub token_saved_code: fn(code: &str) -> String,
    pub token_saved_probe_fail: fn(err: &str) -> String,
    pub token_removed: fn(profile: &str) -> String,
    pub profile_not_found: fn(profile: &str) -> String,
    pub access_ok: &'static str,
    pub access_invalid: &'static str,
    pub access_forbidden: &'static str,
    pub access_unexpected: fn(code: &str) -> String,
    pub no_token: fn(profile: &str) -> String,

    // domains.
    pub h_id: &'static str,
    pub h_domain: &'static str,
    pub h_delegated: &'static str,
    pub h_tariff: &'static str,
    pub domain_created: fn(name: &str, id: &str) -> String,
    pub domain_deleted: fn(name: &str) -> String,
    pub domain_not_found: fn(name: &str) -> String,
    pub confirm_delete_domain: fn(name: &str) -> String,
    pub delegation_ok: fn(name: &str) -> String,
    pub delegation_not_delegated: fn(name: &str) -> String,
    pub delegation_current_ns: fn(ns: &str) -> String,
    pub delegation_missing_ns: fn(ns: &str) -> String,
    pub delegation_extra_ns: fn(ns: &str) -> String,
    pub delegation_no_ns: &'static str,
    pub delegation_unknown: &'static str,
    pub delegation_propagation_note: &'static str,

    // dns.
    pub h_name: &'static str,
    pub h_type: &'static str,
    pub h_ttl: &'static str,
    pub h_proxy: &'static str,
    pub h_values: &'static str,
    pub dns_added: fn(name: &str, rr_type: &str, domain: &str, count: &str) -> String,
    pub dns_set: fn(name: &str, rr_type: &str, domain: &str, count: &str) -> String,
    pub dns_removed: fn(name: &str, rr_type: &str, domain: &str, count: &str) -> String,
    pub record_not_found: fn(name: &str, rr_type: &str) -> String,
    pub nothing_to_delete: fn(name: &str, rr_type: &str) -> String,

    // sites.
    pub h_host: &'static str,
    pub h_status: &'static str,
    pub h_version: &'static str,
    pub h_size: &'static str,
    pub h_path: &'static str,
    pub h_etag: &'static str,
    pub site_created: fn(host: &str, id: &str) -> String,
    pub site_disabled: fn(host: &str) -> String,
    pub publish_summary:
        fn(id: &str, local: &str, server: &str, up: &str, del: &str, same: &str) -> String,
    pub publish_no_changes: &'static str,
    pub publish_dry_run: &'static str,
    pub published: &'static str,
    pub bucket_publish_started: &'static str,
    pub bucket_published: fn(version: &str) -> String,
    pub bucket_publish_failed: fn(error: &str) -> String,
    pub bucket_publish_timeout: &'static str,
    pub deleted_files: fn(count: &str) -> String,
    /// `indicatif` progress template — the `{bar}`/`{pos}`/`{len}` placeholders
    /// are filled by the progress bar itself, not by us.
    pub upload_bar: &'static str,
    pub not_found_site: fn(host: &str) -> String,
    pub publish_needs_site_ref: &'static str,
    pub not_a_dir: fn(path: &str) -> String,
    pub dir_not_found: fn(path: &str) -> String,

    // proxy.
    pub h_mode: &'static str,
    pub h_target: &'static str,
    pub h_ssl: &'static str,
    pub h_bot_prot: &'static str,
    pub proxy_created: fn(host: &str) -> String,
    pub proxy_updated: fn(host: &str) -> String,
    pub proxy_removed: fn(host: &str) -> String,
    pub confirm_remove_proxy: fn(host: &str) -> String,
    pub not_found_proxy: fn(host: &str) -> String,

    // stats and billing.
    pub h_currency: &'static str,
    pub h_balance: &'static str,
    pub h_metric: &'static str,
    pub h_ip: &'static str,
    pub h_reason: &'static str,
    pub h_last_seen: &'static str,
    pub h_requests: &'static str,
    pub no_bans: &'static str,

    // API errors.
    pub err_network: &'static str,
    pub err_parse: &'static str,
    pub err_unauthorized: &'static str,
    pub err_forbidden: &'static str,
}
