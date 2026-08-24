//! The set of strings every locale must provide.
//!
//! Fixed strings are `&'static str`, templates are function pointers — the locale
//! files (`locales/*.iro`) supply closures, so a wrong number of placeholders does
//! not compile.

//! The set of strings every locale must provide.

macro_rules! locale_fields {
    (
        $(
            $(#[$meta:meta])*
            $field:ident $((
                $($arg:ident $( : $ty:ty )? ),* $(,)?
            ))?
        ),* $(,)?
    ) => {
        #[derive(Debug)]
        pub struct Locale {
            $(
                $(#[$meta])*
                pub $field: locale_fields!(@type $($($arg $( : $ty )? ),*)?),
            )*
        }
    };

    (@type) => {
        &'static str
    };

    (@type $($arg:ident $( : $ty:ty )? ),+) => {
        fn($($arg: locale_fields!(@arg_ty $( $ty )? )),+) -> String
    };

    (@arg_ty $ty:ty) => { $ty };
    (@arg_ty) => { &str };
}

locale_fields! {
    // Help: application and command groups.
    app_about,
    cmd_auth,
    cmd_domains,
    cmd_dns,
    cmd_sites,
    cmd_proxy,
    cmd_stats,
    cmd_billing,
    cmd_completion,

    // Help: global arguments.
    arg_profile,
    arg_api_url,
    arg_token,
    arg_output,
    arg_output_table,
    arg_output_json,
    arg_yes,
    arg_page,
    arg_shell,
    arg_domain,
    arg_hostname,

    // Help: auth.
    cmd_auth_login,
    cmd_auth_status,
    cmd_auth_logout,
    arg_login_token,
    arg_login_api_url,

    // Help: domains.
    cmd_domains_list,
    cmd_domains_add,
    cmd_domains_get,
    cmd_domains_remove,
    cmd_domains_check,
    arg_domain_name,
    arg_domains_import,

    // Help: dns.
    cmd_dns_list,
    cmd_dns_add,
    cmd_dns_set,
    cmd_dns_remove,
    cmd_dns_dnssec,
    cmd_dnssec_status,
    cmd_dnssec_enable,
    cmd_dnssec_disable,
    arg_record_name,
    arg_record_type,
    arg_record_type_filter,
    arg_record_value,
    arg_record_value_remove,
    arg_record_ttl,
    arg_dnssec_force,

    // Help: sites.
    cmd_sites_list,
    cmd_sites_create,
    cmd_sites_publish,
    cmd_sites_publish_bucket,
    cmd_sites_files,
    cmd_sites_disable,
    arg_site_hostname,
    arg_site_domain,
    arg_publish_hostname,
    arg_publish_site_id,
    arg_publish_dir,
    arg_publish_dry_run,
    arg_bucket,
    arg_bucket_path,

    // Help: proxy.
    cmd_proxy_list,
    cmd_proxy_get,
    cmd_proxy_set,
    cmd_proxy_remove,
    arg_proxy_domain,
    arg_proxy_mode,
    arg_proxy_redirect_target,
    arg_proxy_ssl,
    arg_proxy_bot_protection,
    arg_proxy_captcha,
    arg_proxy_http2,
    arg_proxy_http3,
    arg_proxy_max_body,
    arg_proxy_block_bots,

    // Help: stats and billing.
    cmd_stats_summary,
    cmd_stats_bans,
    arg_range,
    cmd_billing_balance,
    cmd_billing_usage,
    cmd_billing_tariffs,

    // Help: language
    cmd_lang,
    cmd_lang_set,
    cmd_lang_unset,

    // Common.
    yes,
    no,
    dash,
    empty,
    h_value,
    error_prefix,
    confirm_suffix,
    confirm_cancelled,

    // auth.
    token_prompt,
    token_warn_prefix,
    token_empty,
    lbl_profile,
    lbl_api_url,
    lbl_token,
    lbl_lang,
    lbl_access,
    token_set,
    token_unset,
    login_hint,
    token_saved_ok(profile),
    token_saved_scoped(profile),
    token_saved_code(code),
    token_saved_probe_fail(err),
    token_removed(profile),
    profile_not_found(profile),
    access_ok,
    access_invalid,
    access_forbidden,
    access_unexpected(code),
    no_token(profile),

    // domains.
    h_id,
    h_domain,
    h_delegated,
    h_tariff,
    domain_created(name, id),
    domain_deleted(name),
    domain_not_found(name),
    confirm_delete_domain(name),
    delegation_ok(name),
    delegation_not_delegated(name),
    delegation_current_ns(ns),
    delegation_missing_ns(ns),
    delegation_extra_ns(ns),
    delegation_no_ns,
    delegation_unknown,
    delegation_propagation_note,

    // dns.
    h_name,
    h_type,
    h_ttl,
    h_proxy,
    h_values,
    dns_added(name , rr_type, domain, count),
    dns_set(name, rr_type, domain, count),
    dns_removed(name, rr_type, domain, count),
    record_not_found(name, rr_type),
    nothing_to_delete(name, rr_type),

    // sites.
    h_host,
    h_status,
    h_version,
    h_size,
    h_path,
    h_etag,
    site_created(host, id),
    site_disabled(host),
    publish_summary(id, local, server, up, del, same),
    publish_no_changes,
    publish_dry_run,
    published,
    bucket_publish_started,
    bucket_published(version: i64),
    bucket_publish_failed(error),
    bucket_publish_timeout,
    deleted_files(count),

    /// `indicatif` progress template — the `{bar}`/`{pos}`/`{len}` placeholders
    /// are filled by the progress bar itself, not by us.
    upload_bar,
    not_found_site(host),
    publish_needs_site_ref,
    not_a_dir(path),
    dir_not_found(path),

    // proxy.
    h_mode,
    h_target,
    h_ssl,
    h_bot_prot,
    proxy_created(host),
    proxy_updated(host),
    proxy_removed(host),
    confirm_remove_proxy(host),
    not_found_proxy(host),

    // stats and billing.
    h_currency,
    h_balance,
    h_metric,
    h_ip,
    h_reason,
    h_last_seen,
    h_requests,
    no_bans,

    // API errors.
    err_network,
    err_parse,
    err_unauthorized,
    err_forbidden,
}
