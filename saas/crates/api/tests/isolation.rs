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
    let bytes = to_bytes(response.into_body(), 1048576).await.unwrap();
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
    let registration = |mail: &str, tenant: &str| json!({"email":mail,"password":PASSWORD,"password_confirmation":PASSWORD,"name":"Test owner","tenant_name":tenant});
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
        json!({"email":"a@test.invalid","password":"short","password_confirmation":"short","name":"A","tenant_name":"Alpha"}),
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
        json!({"email":"viewer@test.invalid","name":"Viewer","password":PASSWORD,"password_confirmation":PASSWORD}),
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
    catalog_checks(&router, &pool, &a, &b, &viewer, tenant_a, ua, ub).await;
    let parallel_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&app_url)
        .await
        .unwrap();
    let parallel_router = app(parallel_pool.clone(), "web", config());
    entry_checks(
        &parallel_router,
        &parallel_pool,
        &a,
        &b,
        &viewer,
        tenant_a,
        ua,
        ub,
    )
    .await;
    parallel_pool.close().await;
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
        json!({"email":"bad@test.invalid","name":"bad","password":PASSWORD,"password_confirmation":PASSWORD}),
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

#[allow(clippy::too_many_arguments)]
async fn catalog_checks(
    router: &Router,
    pool: &sqlx::PgPool,
    a: &str,
    b: &str,
    viewer: &str,
    tenant: Uuid,
    user: Uuid,
    other_user: Uuid,
) {
    let account = |code: &str, amount: &str| json!({"code":code,"number":"","name":"Banco teste","opening_balance":amount,"active":true});
    let history = |code: &str, direction: i16| json!({"code":code,"description":"Histórico teste","direction":direction,"active":true});
    let mismatch = json!({"email":"mismatch@test.invalid","name":"Mismatch","tenant_name":"Mismatch","password":PASSWORD,"password_confirmation":"different"});
    assert_eq!(
        call(router, "POST", "/api/auth/register", None, mismatch, ORIGIN)
            .await
            .0,
        400
    );
    assert_eq!(call(router,"POST","/api/members",Some(a),json!({"email":"mismatch@test.invalid","name":"Mismatch","password":PASSWORD,"password_confirmation":"different"}),ORIGIN).await.0,400);
    assert_eq!(call(router,"POST","/api/auth/register",None,json!({"email":"missingconfirm@test.invalid","name":"X","tenant_name":"X","password":PASSWORD}),ORIGIN).await.0,422);
    for path in ["/api/accounts", "/api/histories"] {
        assert_eq!(
            call(router, "GET", path, None, Value::Null, ORIGIN).await.0,
            401
        );
        assert_eq!(
            call(router, "GET", path, Some(viewer), Value::Null, ORIGIN)
                .await
                .0,
            200
        );
    }
    for (path, body) in [
        ("/api/accounts", account("1", "0.10")),
        ("/api/histories", history("1", 1)),
    ] {
        assert_eq!(
            call(router, "POST", path, Some(viewer), body.clone(), ORIGIN)
                .await
                .0,
            403
        );
        assert_eq!(
            call(
                router,
                "POST",
                path,
                Some(a),
                body.clone(),
                "https://evil.invalid"
            )
            .await
            .0,
            403
        );
        let mut spoof = body;
        spoof["tenant_id"] = json!(tenant);
        assert_eq!(
            call(router, "POST", path, Some(a), spoof, ORIGIN).await.0,
            422
        );
    }
    let (status, first, _) = call(
        router,
        "POST",
        "/api/accounts",
        Some(a),
        account("1", "0.10"),
        ORIGIN,
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(first["code"], "01");
    assert_eq!(
        call(
            router,
            "POST",
            "/api/accounts",
            Some(b),
            account("01", "99.00"),
            ORIGIN
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            router,
            "POST",
            "/api/accounts",
            Some(a),
            account("01", "1"),
            ORIGIN
        )
        .await
        .0,
        409
    );
    assert_eq!(
        call(
            router,
            "POST",
            "/api/accounts",
            Some(a),
            account("2", "0,20"),
            ORIGIN
        )
        .await
        .0,
        201
    );
    for amount in ["1.001", "1.000,00", "1e2", "90000000000.01"] {
        assert_eq!(
            call(
                router,
                "POST",
                "/api/accounts",
                Some(a),
                account("invalid", amount),
                ORIGIN
            )
            .await
            .0,
            400
        );
    }
    let path = format!("/api/accounts/{}", first["id"].as_str().unwrap());
    let mut edit = account("1", "-0.10");
    edit["expected_version"] = json!(1);
    assert_eq!(
        call(router, "GET", &path, Some(b), Value::Null, ORIGIN)
            .await
            .0,
        404
    );
    assert_eq!(
        call(router, "PUT", &path, Some(b), edit.clone(), ORIGIN)
            .await
            .0,
        404
    );
    assert_eq!(
        call(router, "PUT", &path, Some(viewer), edit.clone(), ORIGIN)
            .await
            .0,
        403
    );
    assert_eq!(
        call(router, "GET", "/api/accounts", Some(a), Value::Null, ORIGIN)
            .await
            .1["active_opening_balance"],
        "0.30"
    );
    let (left, right) = tokio::join!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN),
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
    );
    assert!((left.0 == 200 && right.0 == 409) || (left.0 == 409 && right.0 == 200));
    assert_eq!(
        call(router, "GET", "/api/accounts", Some(a), Value::Null, ORIGIN)
            .await
            .1["active_opening_balance"],
        "0.10"
    );
    edit["expected_version"] = json!(2);
    edit["active"] = json!(false);
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
            .await
            .0,
        200
    );
    assert_eq!(
        call(router, "GET", "/api/accounts", Some(a), Value::Null, ORIGIN)
            .await
            .1["active_opening_balance"],
        "0.20"
    );
    assert_eq!(
        call(
            router,
            "POST",
            "/api/accounts",
            Some(a),
            account("1", "0"),
            ORIGIN
        )
        .await
        .0,
        409
    );
    edit["expected_version"] = json!(3);
    edit["active"] = json!(true);
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
            .await
            .0,
        200
    );
    edit["expected_version"] = json!(4);
    edit["code"] = json!("02");
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit, ORIGIN).await.0,
        409
    );
    assert_eq!(
        call(router, "GET", &path, Some(a), Value::Null, ORIGIN)
            .await
            .1["version"],
        4
    );
    let (status, h, _) = call(
        router,
        "POST",
        "/api/histories",
        Some(a),
        history("income", 1),
        ORIGIN,
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(
        call(
            router,
            "POST",
            "/api/histories",
            Some(b),
            history("income", -1),
            ORIGIN
        )
        .await
        .0,
        201
    );
    assert_eq!(
        call(
            router,
            "POST",
            "/api/histories",
            Some(a),
            history("INCOME", 1),
            ORIGIN
        )
        .await
        .0,
        409
    );
    assert_eq!(
        call(
            router,
            "POST",
            "/api/histories",
            Some(a),
            history("invalid", 0),
            ORIGIN
        )
        .await
        .0,
        400
    );
    let path = format!("/api/histories/{}", h["id"].as_str().unwrap());
    let mut edit = history("income", -1);
    edit["expected_version"] = json!(1);
    edit["active"] = json!(false);
    assert_eq!(
        call(router, "GET", &path, Some(b), Value::Null, ORIGIN)
            .await
            .0,
        404
    );
    assert_eq!(
        call(router, "PUT", &path, Some(b), edit.clone(), ORIGIN)
            .await
            .0,
        404
    );
    assert_eq!(
        call(router, "PUT", &path, Some(viewer), edit.clone(), ORIGIN)
            .await
            .0,
        403
    );
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
            .await
            .0,
        200
    );
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
            .await
            .0,
        409
    );
    edit["expected_version"] = json!(2);
    edit["active"] = json!(true);
    let result = call(router, "PUT", &path, Some(a), edit, ORIGIN).await;
    assert_eq!(result.0, 200);
    assert_eq!(result.1["direction"], -1);
    for table in ["accounts", "histories"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "no leaked tenant context");
        let mut tx = pool.begin().await.unwrap();
        set_context(&mut tx, tenant, user).await.unwrap();
        let wrong: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE tenant_id<>$1"))
                .bind(tenant)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(wrong, 0);
        let err = sqlx::query(&format!("UPDATE {table} SET updated_by=$1"))
            .bind(other_user)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("23503")
        );
        tx.rollback().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        set_context(&mut tx, tenant, user).await.unwrap();
        let err = sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
        tx.rollback().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        set_context(&mut tx, tenant, user).await.unwrap();
        let err = sqlx::query(&format!("UPDATE {table} SET tenant_id=$1"))
            .bind(Uuid::new_v4())
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
        tx.rollback().await.unwrap();
    }
    println!("PASS cycle 3: confirmation, accounts/histories, exact cents, versions/concurrent writes, deactivation, viewer, CSRF, tenant isolation and direct RLS");
}

fn entry_input(account: &str, history: &str, date: &str, amount: &str, cleared: bool) -> Value {
    json!({"account_id":account,"history_id":history,"date":date,"amount":amount,"document":"Test","description":"Audit","cleared":cleared,"archived":false,"request_id":Uuid::new_v4()})
}
async fn balances(router: &Router, cookie: &str, account: &str, real: &str, bank: &str) {
    let (status, value, _) = call(
        router,
        "GET",
        &format!("/api/entries?account_id={account}"),
        Some(cookie),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["current_balance"], real);
    assert_eq!(value["bank_balance"], bank);
}
#[allow(clippy::too_many_arguments)]
async fn entry_checks(
    router: &Router,
    pool: &sqlx::PgPool,
    a: &str,
    b: &str,
    viewer: &str,
    tenant: Uuid,
    user: Uuid,
    other: Uuid,
) {
    let mut accounts = Vec::new();
    for (cookie, code, amount) in [(a, "A4", "1000"), (a, "B4", "500"), (b, "C4", "999")] {
        let (status,v,_)=call(router,"POST","/api/accounts",Some(cookie),json!({"code":code,"number":"","name":"Cycle 4","opening_balance":amount,"active":true}),ORIGIN).await;
        assert_eq!(status, 201);
        accounts.push(v["id"].as_str().unwrap().to_owned());
    }
    let mut histories = Vec::new();
    for (cookie, code, direction) in [(a, "99", 1), (a, "OUT", -1), (b, "99", 1)] {
        let (status, v, _) = call(
            router,
            "POST",
            "/api/histories",
            Some(cookie),
            json!({"code":code,"description":"Cycle 4","direction":direction,"active":true}),
            ORIGIN,
        )
        .await;
        assert_eq!(status, 201);
        histories.push(v["id"].as_str().unwrap().to_owned());
    }
    let ac = &accounts[0];
    let target = &accounts[1];
    let foreign = &accounts[2];
    let hi = &histories[0];
    let ho = &histories[1];
    let url = format!("/api/entries?account_id={ac}");
    assert_eq!(
        call(router, "GET", &url, None, Value::Null, ORIGIN).await.0,
        401
    );
    assert_eq!(
        call(router, "GET", &url, Some(b), Value::Null, ORIGIN)
            .await
            .0,
        404
    );
    assert_eq!(
        call(router, "GET", &url, Some(viewer), Value::Null, ORIGIN)
            .await
            .0,
        200
    );
    balances(router, a, ac, "1000.00", "1000.00").await;
    let income = entry_input(ac, hi, "2026-10-01", "200", true);
    for (cookie, origin, status) in [
        (Some(viewer), ORIGIN, 403),
        (Some(a), "https://evil.invalid", 403),
        (None, ORIGIN, 401),
    ] {
        assert_eq!(
            call(
                router,
                "POST",
                "/api/entries",
                cookie,
                income.clone(),
                origin
            )
            .await
            .0,
            status
        );
    }
    let mut spoof = income.clone();
    spoof["tenant_id"] = json!(tenant);
    assert_eq!(
        call(router, "POST", "/api/entries", Some(a), spoof, ORIGIN)
            .await
            .0,
        422
    );
    for (acc, hist) in [
        (foreign.as_str(), hi.as_str()),
        (ac.as_str(), histories[2].as_str()),
    ] {
        assert_eq!(
            call(
                router,
                "POST",
                "/api/entries",
                Some(a),
                entry_input(acc, hist, "2026-10-01", "1", false),
                ORIGIN
            )
            .await
            .0,
            404
        );
    }
    for amount in ["0", "-1", "1.001", "90000000000.01"] {
        assert_eq!(
            call(
                router,
                "POST",
                "/api/entries",
                Some(a),
                entry_input(ac, hi, "2026-10-01", amount, false),
                ORIGIN
            )
            .await
            .0,
            400
        );
    }
    for date in ["2026-02-29", "2026-13-01", "0000-01-01"] {
        assert_eq!(
            call(
                router,
                "POST",
                "/api/entries",
                Some(a),
                entry_input(ac, hi, date, "1", false),
                ORIGIN
            )
            .await
            .0,
            400
        );
    }
    let (left, right) = tokio::join!(
        call(
            router,
            "POST",
            "/api/entries",
            Some(a),
            income.clone(),
            ORIGIN
        ),
        call(
            router,
            "POST",
            "/api/entries",
            Some(a),
            income.clone(),
            ORIGIN
        )
    );
    assert_eq!(left.0, 201);
    assert_eq!(right.0, 201);
    assert_eq!(left.1["id"], right.1["id"]);
    let incoming = left.1;
    let mut different = income.clone();
    different["amount"] = json!("201");
    assert_eq!(
        call(router, "POST", "/api/entries", Some(a), different, ORIGIN)
            .await
            .0,
        409
    );
    let expense = entry_input(ac, ho, "2026-10-14", "150", false);
    let (status, out, _) = call(
        router,
        "POST",
        "/api/entries",
        Some(a),
        expense.clone(),
        ORIGIN,
    )
    .await;
    assert_eq!(status, 201);
    balances(router, a, ac, "1050.00", "1200.00").await;
    let path = format!("/api/entries/{}", out["id"].as_str().unwrap());
    assert_eq!(
        call(router, "GET", &path, Some(b), Value::Null, ORIGIN)
            .await
            .0,
        404
    );
    let mut edit = expense.clone();
    edit.as_object_mut().unwrap().remove("request_id");
    edit["expected_version"] = json!(1);
    edit["cleared"] = json!(true);
    assert_eq!(
        call(router, "PUT", &path, Some(b), edit.clone(), ORIGIN)
            .await
            .0,
        404
    );
    assert_eq!(
        call(router, "PUT", &path, Some(viewer), edit.clone(), ORIGIN)
            .await
            .0,
        403
    );
    let (left, right) = tokio::join!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN),
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
    );
    assert!((left.0 == 200 && right.0 == 409) || (left.0 == 409 && right.0 == 200));
    balances(router, a, ac, "1050.00", "1050.00").await;
    edit["expected_version"] = json!(2);
    edit["cleared"] = json!(false);
    edit["amount"] = json!("175");
    edit["archived"] = json!(true);
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
            .await
            .0,
        200
    );
    balances(router, a, ac, "1025.00", "1200.00").await;
    let (status, v, _) = call(
        router,
        "GET",
        &format!("{url}&from=2026-10-14&to=2026-10-14"),
        Some(a),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(v["previous_balance"], "1200.00");
    assert_eq!(v["period_expense"], "175.00");
    assert_eq!(v["period_balance"], "1025.00");
    assert_eq!(v["total"], 0);
    let v = call(
        router,
        "GET",
        &format!("{url}&include_archived=true"),
        Some(a),
        Value::Null,
        ORIGIN,
    )
    .await
    .1;
    assert_eq!(v["total"], 2);
    assert_eq!(v["items"][0]["id"], incoming["id"]);
    assert_eq!(
        call(
            router,
            "GET",
            &format!("{url}&from=2026-10-15&to=2026-10-01"),
            Some(a),
            Value::Null,
            ORIGIN
        )
        .await
        .0,
        400
    );
    // Changing catalog financial meaning after use must fail without changing balances.
    assert_eq!(call(router,"PUT",&format!("/api/accounts/{ac}"),Some(a),json!({"code":"A4","number":"","name":"Cycle 4","opening_balance":"999","active":true,"expected_version":1}),ORIGIN).await.0,409);
    assert_eq!(call(router,"PUT",&format!("/api/histories/{ho}"),Some(a),json!({"code":"OUT","description":"Cycle 4","direction":1,"active":true,"expected_version":1}),ORIGIN).await.0,409);
    edit["expected_version"] = json!(3);
    edit["account_id"] = json!(target);
    edit["history_id"] = json!(hi);
    edit["archived"] = json!(false);
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit.clone(), ORIGIN)
            .await
            .0,
        200
    );
    balances(router, a, ac, "1200.00", "1200.00").await;
    balances(router, a, target, "675.00", "500.00").await;
    let cancel = format!("{path}/cancel");
    assert_eq!(
        call(
            router,
            "POST",
            &cancel,
            Some(b),
            json!({"expected_version":4}),
            ORIGIN
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            router,
            "POST",
            &cancel,
            Some(viewer),
            json!({"expected_version":4}),
            ORIGIN
        )
        .await
        .0,
        403
    );
    assert_eq!(
        call(
            router,
            "POST",
            &cancel,
            Some(a),
            json!({"expected_version":3}),
            ORIGIN
        )
        .await
        .0,
        409
    );
    assert_eq!(
        call(
            router,
            "POST",
            &cancel,
            Some(a),
            json!({"expected_version":4}),
            ORIGIN
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            router,
            "POST",
            &cancel,
            Some(a),
            json!({"expected_version":5}),
            ORIGIN
        )
        .await
        .0,
        200
    );
    edit["expected_version"] = json!(5);
    assert_eq!(
        call(router, "PUT", &path, Some(a), edit, ORIGIN).await.0,
        409
    );
    balances(router, a, target, "500.00", "500.00").await;
    let replay = call(router, "POST", "/api/entries", Some(a), expense, ORIGIN).await;
    assert_eq!(replay.0, 201);
    assert_eq!(replay.1["cancelled"], true);
    balances(router, a, target, "500.00", "500.00").await;
    let v = call(
        router,
        "GET",
        &format!("/api/entries?account_id={target}&include_cancelled=true"),
        Some(a),
        Value::Null,
        ORIGIN,
    )
    .await
    .1;
    assert_eq!(v["total"], 1);
    // Inactive references can be retained for corrections but cannot be newly selected.
    assert_eq!(call(router,"PUT",&format!("/api/accounts/{ac}"),Some(a),json!({"code":"A4","number":"","name":"Cycle 4","opening_balance":"1000","active":false,"expected_version":1}),ORIGIN).await.0,200);
    assert_eq!(
        call(
            router,
            "POST",
            "/api/entries",
            Some(a),
            entry_input(ac, hi, "2026-10-14", "1", false),
            ORIGIN
        )
        .await
        .0,
        409
    );
    let mut edit = income;
    edit.as_object_mut().unwrap().remove("request_id");
    edit["expected_version"] = json!(1);
    edit["description"] = json!("Correction on inactive account");
    assert_eq!(
        call(
            router,
            "PUT",
            &format!("/api/entries/{}", incoming["id"].as_str().unwrap()),
            Some(a),
            edit,
            ORIGIN
        )
        .await
        .0,
        200
    );
    balances(router, a, ac, "1200.00", "1200.00").await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM entries")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    for (sql, value, expected) in [
        (
            "UPDATE entries SET account_id=$1",
            Uuid::parse_str(foreign).unwrap(),
            "23503",
        ),
        (
            "UPDATE entries SET history_id=$1",
            Uuid::parse_str(&histories[2]).unwrap(),
            "23503",
        ),
        ("UPDATE entries SET updated_by=$1", other, "23503"),
        ("UPDATE entries SET tenant_id=$1", Uuid::new_v4(), "42501"),
    ] {
        let mut tx = pool.begin().await.unwrap();
        set_context(&mut tx, tenant, user).await.unwrap();
        let err = sqlx::query(sql)
            .bind(value)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some(expected)
        );
        tx.rollback().await.unwrap();
    }
    let mut tx = pool.begin().await.unwrap();
    set_context(&mut tx, tenant, user).await.unwrap();
    let err = sqlx::query("DELETE FROM entries")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        err.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    tx.rollback().await.unwrap();
    // Pagination must not truncate totals or duplicate same-day rows.
    let mut page_ids = Vec::new();
    for _ in 0..201 {
        let r = call(
            router,
            "POST",
            "/api/entries",
            Some(a),
            entry_input(target, hi, "2026-10-20", "0.01", true),
            ORIGIN,
        )
        .await;
        assert_eq!(r.0, 201);
        page_ids.push(r.1["id"].clone());
    }
    let endpoint = format!("/api/entries?account_id={target}&from=2026-10-20&to=2026-10-20");
    let r = call(router, "GET", &endpoint, Some(a), Value::Null, ORIGIN).await;
    assert_eq!(r.0, 200);
    assert_eq!(r.1["items"].as_array().unwrap().len(), 200);
    assert_eq!(r.1["total"], 201);
    assert_eq!(r.1["next_offset"], 200);
    assert_eq!(r.1["period_income"], "2.01");
    assert_eq!(r.1["current_balance"], "502.01");
    assert_eq!(r.1["bank_balance"], "502.01");
    for (index, item) in r.1["items"].as_array().unwrap().iter().enumerate() {
        assert_eq!(item["id"], page_ids[index]);
    }
    let r = call(
        router,
        "GET",
        &format!("{endpoint}&offset=200"),
        Some(a),
        Value::Null,
        ORIGIN,
    )
    .await;
    assert_eq!(r.1["items"].as_array().unwrap().len(), 1);
    assert_eq!(r.1["items"][0]["id"], page_ids[200]);
    assert!(r.1["next_offset"].is_null());
    assert_eq!(r.1["current_balance"], "502.01");

    println!("PASS cycle 4: financial balances, dates, edits/moves, clearing, archive/cancel, idempotency, concurrent versions, inactive references and cross-tenant RLS/FKs");
}
