#[cfg(test)]
mod tests {
    use crate::commands::sites::*;
    use serde_json::{json, Value};
    use wiremock::matchers::{body_json, method, path as url_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn md5_hex(data: &[u8]) -> String {
        let mut hasher = Md5::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn touch(dir: &Path, rel: &str, content: &[u8]) -> PathBuf {
        let abs = dir.join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&abs, content).unwrap();
        abs
    }

    #[test]
    fn make_batches_splits_by_file_count() {
        let dir = tempfile::tempdir().unwrap();
        let files: Vec<(String, PathBuf)> = (0..250)
            .map(|i| {
                let rel = format!("f{i}.txt");
                let abs = touch(dir.path(), &rel, b"");
                (rel, abs)
            })
            .collect();
        let batches = make_batches(files);
        let sizes: Vec<usize> = batches.iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![100, 100, 50]);
    }

    #[test]
    fn make_batches_splits_by_total_size() {
        let dir = tempfile::tempdir().unwrap();
        // Sparse 20 MB files: two of them exceed the 32 MB batch ceiling.
        let files: Vec<(String, PathBuf)> = (0..3)
            .map(|i| {
                let rel = format!("big{i}.bin");
                let abs = dir.path().join(&rel);
                let f = std::fs::File::create(&abs).unwrap();
                f.set_len(20 * 1024 * 1024).unwrap();
                (rel, abs)
            })
            .collect();
        let batches = make_batches(files);
        let sizes: Vec<usize> = batches.iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![1, 1, 1]);
    }

    #[test]
    fn make_batches_of_nothing_is_empty() {
        assert!(make_batches(Vec::new()).is_empty());
    }

    fn client(server: &MockServer) -> Client<'_> {
        Client::new(&server.uri(), "wsk_test").unwrap()
    }

    /// Mounts `GET files` returning the given draft state.
    async fn mount_files(server: &MockServer, files: Value) {
        Mock::given(method("GET"))
            .and(url_path("/api/v1/static-sites/5/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "files": files })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn publish_uploads_changed_deletes_vanished_then_publishes() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "index.html", b"<html>new</html>");
        touch(dir.path(), "css/app.css", b"body{}");
        let same = b"unchanged";
        touch(dir.path(), "same.txt", same);

        let server = MockServer::start().await;
        // Draft on the server: index.html is stale, same.txt matches its local
        // MD5 (must be skipped), old.txt vanished locally (must be deleted).
        mount_files(
            &server,
            json!([
                {"path": "index.html", "etag": "0000stale0000"},
                {"path": "same.txt", "etag": md5_hex(same)},
                {"path": "old.txt", "etag": "aaaa"},
            ]),
        )
        .await;
        // index.html + css/app.css fit one batch → exactly one upload request.
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/upload"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/delete-files"))
            .and(body_json(json!({"paths": ["old.txt"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/publish"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;

        publish(&client(&server), 5, dir.path(), false)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_dry_run_only_reads() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "index.html", b"data");

        let server = MockServer::start().await;
        mount_files(&server, json!([])).await;
        // No POST mocks are mounted: any write attempt would 404 and fail the run.
        publish(&client(&server), 5, dir.path(), true)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_skips_snapshot_when_nothing_changed() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"stable";
        touch(dir.path(), "index.html", content);

        let server = MockServer::start().await;
        mount_files(
            &server,
            json!([{"path": "index.html", "etag": md5_hex(content)}]),
        )
        .await;
        publish(&client(&server), 5, dir.path(), false)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_fails_on_missing_directory() {
        let server = MockServer::start().await;
        let missing = std::env::temp_dir().join("webshield-cli-no-such-dir-xyz");
        assert!(publish(&client(&server), 5, &missing, true).await.is_err());
    }

    #[tokio::test]
    async fn publish_from_bucket_posts_body_and_polls_until_active() {
        let server = MockServer::start().await;
        // The action posts {bucket, path} and returns 202 with status "publishing".
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/publish-from-bucket"))
            .and(body_json(json!({"bucket": "web", "path": "public/"})))
            .respond_with(
                ResponseTemplate::new(202)
                    .set_body_json(json!({"id": 5, "hostname": "s", "status": "publishing"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        // Polling the site returns "active" → success.
        Mock::given(method("GET"))
            .and(url_path("/api/v1/static-sites/5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"id": 5, "hostname": "s", "status": "active", "content_version": 7}),
            ))
            .mount(&server)
            .await;

        publish_from_bucket(&client(&server), 5, "web", "public/")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn publish_from_bucket_surfaces_publish_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(url_path("/api/v1/static-sites/5/publish-from-bucket"))
            .respond_with(
                ResponseTemplate::new(202)
                    .set_body_json(json!({"id": 5, "hostname": "s", "status": "publishing"})),
            )
            .mount(&server)
            .await;
        // Async publish failed: status reverted, publish_error carries the reason.
        Mock::given(method("GET"))
            .and(url_path("/api/v1/static-sites/5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"id": 5, "hostname": "s", "status": "disabled", "publish_error": "boom"}),
            ))
            .mount(&server)
            .await;

        let err = publish_from_bucket(&client(&server), 5, "web", "")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("boom"));
    }
}
