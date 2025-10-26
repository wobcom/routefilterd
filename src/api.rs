use crate::store;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::{Router, routing::get};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

#[derive(Deserialize)]
struct PoofParameter {
    name: String,
    recursion_depth: Option<u32>,
}

async fn get_route_from_as_set(
    State(data): State<Arc<store::PoofStore>>,
    Query(query): Query<PoofParameter>,
) -> impl IntoResponse {
    let name = query.name.clone();
    let recursion_depth = query.recursion_depth.or(Some(32)).unwrap();
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_prefixes_recursive(name.to_string(), recursion_depth) {
        return format!(
            "# Recursed route resolution for '{}' in {}μs, {} items\n{:#?}",
            name,
            old_time.elapsed().as_micros(),
            value.len(),
            value
        );
    } else {
        return format!("Value for '{}' not found in cache.", name);
    }
}

async fn get_asn_from_as_set(
    State(data): State<Arc<store::PoofStore>>,
    Query(query): Query<PoofParameter>,
) -> impl IntoResponse {
    let name = query.name.clone();
    let recursion_depth = query.recursion_depth.or(Some(32)).unwrap();
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_recursive(name.to_string(), recursion_depth) {
        return format!(
            "# Recursed AS-Set resolution for '{}' in {}μs, {} items\n{:#?}",
            name,
            old_time.elapsed().as_micros(),
            value.len(),
            value
        );
    } else {
        return format!("Value for '{}' not found in cache.", name);
    }
}

pub async fn listen(listen_address: String, store: Arc<store::PoofStore>) {
    let routermake = Router::new()
        .route("/asnsFromAsSet", get(get_asn_from_as_set))
        .route("/routesFromAsSet", get(get_route_from_as_set))
        .with_state(store.clone());

    let mut router = Router::new();

    router = router.nest("/api/v1", routermake);

    let make_service = router.into_make_service();

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(listen_address).await.unwrap();
    axum::serve(listener, make_service).await.unwrap();
}
