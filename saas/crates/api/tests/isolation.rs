use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use contas_api::{app, auth::set_context, Config};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, Row};
use tower::ServiceExt;
use uuid::Uuid;

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Value,
    origin: &str,
) -> (u16, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("origin", origin)
        .header("x-contas-request", "1")
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(if method == "GET" {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body, cookie)
}
const ORIGIN: &str = "http://127.0.0.1:8080";
const PASSWORD: &str = "Test-password-2026!";
fn config() -> Config {
    Config {
        origin: ORIGIN.into(),
        secure_cookie: false,
        registration: true,
    }
}

#[tokio::test]
#[ignore = "Run scripts/Test-Cycle2.ps1 against its disposable PostgreSQL cluster"]
async fn authentication_and_tenant_isolation() {
    assert_eq!(std::env::var("TEST_DISPOSABLE").as_deref(), Ok("1"));
    let admin_url = std::env::var("TEST_ADMIN_URL").unwrap();
    let app_url = std::env::var("TEST_APP_URL").unwrap();
    assert!(admin_url.contains("@127.0.0.1:"));
    assert!(app_url.contains("@127.0.0.1:"));
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../migrations"
    )))
    .await
    .unwrap();
    migrator.run(&admin).await.unwrap();
    migrator.run(&admin).await.unwrap();
    sqlx::query("ALTER ROLE contas_app LOGIN")
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&app_url)
        .await
        .unwrap();
    contas_api::check_runtime_role(&pool).await.unwrap();
    assert!(contas_api::check_runtime_role(&admin).await.is_err());
    let router = app(pool.clone(), "web", config());
    let (code, _, _) = call(&router, "GET", "/api/auth/me", None, Value::Null, ORIGIN).await;
    assert_eq!(code, 401);
    let registration = |mail: &str, tenant: &str| json!({"email":mail,"password":PASSWORD,"name":"Test owner","tenant_name":tenant});
    let (code, _, _) = call(
        &router,
        "POST",
        "/api/auth/register",
        None,
        registration("a@test.invalid", "Alpha"),
        "https://evil.invalid",
    )
    .await;
    assert_eq!(code, 403);
    let (code, _, _) = call(
        &router,
        "POST",
        "/api/auth/register",
        None,
        json!({"email":"a@test.invalid","password":"short","name":"A","tenant_name":"Alpha"}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 400);
    let (code, _, a) = call(
        &router,
        "POST",
        "/api/auth/register",
        None,
        registration("A@Test.Invalid", "Alpha"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 201);
    let a = a.unwrap();
    assert!(a.contains("HttpOnly") && a.contains("SameSite=Strict") && a.contains("Max-Age=43200"));
    let (code, _, b) = call(
        &router,
        "POST",
        "/api/auth/register",
        None,
        registration("b@test.invalid", "Beta"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 201);
    let b = b.unwrap();
    let (_, ma, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&a),
        Value::Null,
        ORIGIN,
    )
    .await;
    let (_, mb, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&b),
        Value::Null,
        ORIGIN,
    )
    .await;
    let ta = ma["tenant_id"].as_str().unwrap();
    let tb = mb["tenant_id"].as_str().unwrap();
    assert_ne!(ta, tb);
    assert_eq!(ma["email"], "a@test.invalid");
    let (code, _, _) = call(
        &router,
        "POST",
        "/api/auth/register",
        None,
        registration("a@test.invalid", "Duplicate"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 409);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tenants")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(count, 2, "duplicate must roll back tenant creation");
    for (cookie, own, other) in [(&a, ta, tb), (&b, tb, ta)] {
        let (code, _, _) = call(
            &router,
            "GET",
            &format!("/api/tenants/{own}"),
            Some(cookie),
            Value::Null,
            ORIGIN,
        )
        .await;
        assert_eq!(code, 200);
        let (code, _, _) = call(
            &router,
            "GET",
            &format!("/api/tenants/{other}"),
            Some(cookie),
            Value::Null,
            ORIGIN,
        )
        .await;
        assert_eq!(code, 404);
        let (code, _, _) = call(
            &router,
            "PUT",
            &format!("/api/tenants/{other}"),
            Some(cookie),
            json!({"name":"Invaded"}),
            ORIGIN,
        )
        .await;
        assert_eq!(code, 404);
    }
    let (code, _, _) = call(
        &router,
        "PUT",
        &format!("/api/tenants/{ta}"),
        Some(&a),
        json!({"name":"Alpha updated"}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 204);
    let (code, _, _) = call(
        &router,
        "PUT",
        &format!("/api/tenants/{ta}"),
        Some(&a),
        json!({"name":"Spoof","tenant_id":tb}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 422);
    let (code, _, _) = call(
        &router,
        "POST",
        "/api/auth/logout",
        Some(&a),
        json!({}),
        "https://evil.invalid",
    )
    .await;
    assert_eq!(code, 403);
    let missing_header = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("origin", ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_header.status(), StatusCode::FORBIDDEN);
    for _ in 0..12 {
        let (ra, rb) = tokio::join!(
            call(
                &router,
                "GET",
                "/api/auth/me",
                Some(&a),
                Value::Null,
                ORIGIN
            ),
            call(
                &router,
                "GET",
                "/api/auth/me",
                Some(&b),
                Value::Null,
                ORIGIN
            )
        );
        assert_eq!(ra.1["tenant_id"], ta);
        assert_eq!(rb.1["tenant_id"], tb);
    }
    println!("PASS API cross-tenant read/write, spoofing, CSRF and concurrent pool reuse");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tenants")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "pool must not retain tenant context");
    let ua = Uuid::parse_str(ma["user_id"].as_str().unwrap()).unwrap();
    let ub = Uuid::parse_str(mb["user_id"].as_str().unwrap()).unwrap();
    let tenant_a = Uuid::parse_str(ta).unwrap();
    let tenant_b = Uuid::parse_str(tb).unwrap();
    let mut tx = pool.begin().await.unwrap();
    set_context(&mut tx, tenant_a, ua).await.unwrap();
    let rows = sqlx::query("SELECT id FROM tenants")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Uuid, _>("id"), tenant_a);
    let changed = sqlx::query("UPDATE tenants SET name='Wrong' WHERE id=$1")
        .bind(tenant_b)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(changed.rows_affected(), 0);
    let error = sqlx::query("INSERT INTO tenants(id,name) VALUES($1,'Wrong')")
        .bind(Uuid::new_v4())
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    set_context(&mut tx, tenant_a, ua).await.unwrap();
    let error = sqlx::query("UPDATE tenant_settings SET updated_by=$1 WHERE tenant_id=$2")
        .bind(ub)
        .bind(tenant_a)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    tx.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tenant_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    println!("PASS direct RLS, rejected writes, composite foreign key and rollback cleanup");
    let (code, _, _) = call(
        &router,
        "POST",
        "/api/members",
        Some(&a),
        json!({"email":"viewer@test.invalid","name":"Viewer","password":PASSWORD}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 201);
    let login = |mail: &str, password: &str| json!({"email":mail,"password":password});
    let (code, _, viewer) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        login("viewer@test.invalid", PASSWORD),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 200);
    let viewer = viewer.unwrap();
    let (_, mv, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&viewer),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(mv["tenant_id"], ta);
    assert_eq!(mv["role"], "viewer");
    let (code, _, _) = call(
        &router,
        "PUT",
        &format!("/api/tenants/{ta}"),
        Some(&viewer),
        json!({"name":"Forbidden"}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 403);
    let (code, _, _) = call(
        &router,
        "POST",
        "/api/members",
        Some(&viewer),
        json!({"email":"bad@test.invalid","name":"bad","password":PASSWORD}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 403);
    let (code, _, _) = call(
        &router,
        "GET",
        &format!("/api/tenants/{tb}"),
        Some(&viewer),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(code, 404);
    let (code, bad, _) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        login("a@test.invalid", "wrong"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 401);
    let (code, missing, _) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        login("missing@test.invalid", "wrong"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 401);
    assert_eq!(bad, missing);
    let (code, _, fresh) = call(
        &router,
        "POST",
        "/api/auth/login",
        Some(&a),
        login("a@test.invalid", PASSWORD),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 200);
    let fresh = fresh.unwrap();
    assert_ne!(a, fresh);
    let (code, _, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&a),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(code, 401, "old token must be revoked");
    let (code, _, expired) = call(
        &router,
        "POST",
        "/api/auth/logout",
        Some(&fresh),
        json!({}),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 204);
    assert!(expired.unwrap().contains("Max-Age=0"));
    let (code, _, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&fresh),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(code, 401);
    sqlx::query("UPDATE sessions SET expires_at=now()-interval '1 second' WHERE user_id=$1")
        .bind(ub)
        .execute(&admin)
        .await
        .unwrap();
    let (code, _, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&b),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(code, 401);
    sqlx::query("UPDATE users SET active=false WHERE email='viewer@test.invalid'")
        .execute(&admin)
        .await
        .unwrap();
    let (code, _, _) = call(
        &router,
        "GET",
        "/api/auth/me",
        Some(&viewer),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(code, 401);
    let stored: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(ua)
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!(stored.starts_with("$argon2id$"));
    assert!(!stored.contains(PASSWORD));
    let raw_cookie = b.split(';').next().unwrap().split('=').nth(1).unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE token_hash=$1")
        .bind(raw_cookie)
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(count, 0);
    for n in 1..=11 {
        let (code, _, _) = call(
            &router,
            "POST",
            "/api/auth/login",
            None,
            login("limited@test.invalid", "wrong"),
            ORIGIN,
        )
        .await;
        assert_eq!(code, if n <= 10 { 401 } else { 429 });
    }
    let another = app(pool.clone(), "web", config());
    let (code, _, _) = call(
        &another,
        "POST",
        "/api/auth/login",
        None,
        login("limited@test.invalid", "wrong"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 429, "rate limit shared by replicas");
    let secure = app(
        pool.clone(),
        "web",
        Config {
            secure_cookie: true,
            registration: false,
            ..config()
        },
    );
    let (code, _, _) = call(
        &secure,
        "POST",
        "/api/auth/register",
        None,
        registration("closed@test.invalid", "Closed"),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 403);
    let (code, _, cookie) = call(
        &secure,
        "POST",
        "/api/auth/login",
        None,
        login("a@test.invalid", PASSWORD),
        ORIGIN,
    )
    .await;
    assert_eq!(code, 200);
    assert!(cookie.unwrap().contains("; Secure"));
    println!("PASS viewer permissions, password hashing, session rotation/revocation/expiry, disabled users, shared throttling and secure cookie");
    pool.close().await;
    admin.close().await;
}
