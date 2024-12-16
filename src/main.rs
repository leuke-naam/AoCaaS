use std::{collections::HashMap, convert::Infallible};

use axum::{
    async_trait,
    extract::{FromRequestParts, Path, Query, State},
    http::request::Parts,
    response::IntoResponse,
    routing::{any, get},
    Json, RequestPartsExt, Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use reqwest::{header::COOKIE, Client, Url};
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt};

const DOMAIN: &'static str = "https://adventofcode.com";

#[derive(Clone)]
struct Context {
    client: Client,
}

struct Session(String);

#[async_trait]
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        // Extract the bearer token from the authorization header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .unwrap();

        let session = bearer.token().to_owned();

        Ok(Self(session))
    }
}

#[axum::debug_handler]
async fn health_check() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}

#[axum::debug_handler]
async fn get_leaderboard(
    State(context): State<Context>,
    Path(leaderboard): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    session: Session,
) -> impl IntoResponse {
    let Context { client } = context;

    let year = params
        .get("year")
        .cloned()
        .map(|year| year.parse::<i32>().unwrap())
        .unwrap_or(OffsetDateTime::now_utc().year());

    let url = format!("{DOMAIN}/{year}/leaderboard/private/view/{leaderboard}.json")
        .parse::<Url>()
        .unwrap();

    let Session(session) = session;

    let response = client
        .get(url)
        .header(COOKIE, format!("session={session}"))
        .send()
        .await
        .unwrap();

    let body = response.json::<Value>().await.unwrap();

    Json(body)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = Client::new();

    let state = Context { client };

    let app = Router::new()
        .route("/health", any(health_check))
        .route("/leaderboard/:leaderboard", get(get_leaderboard))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = "[::]:3000";

    let listener = TcpListener::bind(addr).await?;

    tracing::debug!(
        "listening on {addr} 🚀",
        addr = listener.local_addr().unwrap()
    );

    axum::serve(listener, app).await?;

    Ok(())
}
