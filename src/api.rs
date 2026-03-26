use crate::ConfigAPI;
use crate::store;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::{Router, routing::get};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
struct APIState {
    config: ConfigAPI,
    store: Arc<store::DataStore>,
}

#[derive(Deserialize)]
struct QueryParameter {
    name: String,
    recursion_depth: Option<u32>,
    #[serde(default)]
    ignore_as_sets: String,
    #[serde(default)]
    datasources: String,
    #[serde(default)]
    ignore_datasource: String,
}

// TODO: Refactor all of these requests

async fn get_route_from_as_set(
    State(state): State<APIState>,
    Query(query): Query<QueryParameter>,
) -> impl IntoResponse {
    let old_time = Instant::now();
    let name = query.name.clone();
    let recursion_depth = query
        .recursion_depth
        .or(Some(state.config.default_recursion_depth))
        .unwrap();
    let ignore_as_sets = query
        .ignore_as_sets
        .clone()
        .split(',')
        .map(|a| a.to_string())
        .filter(|a| a != "")
        .collect::<Vec<String>>();
    let ignore_datasource = query
        .ignore_datasource
        .clone()
        .split(',')
        .map(|a| a.to_string())
        .filter(|a| a != "")
        .collect::<Vec<String>>();
    if let Some(mut value) = state.store.query_as_set_prefixes_recursive(
        name.to_string(),
        recursion_depth,
        ignore_as_sets,
        &ignore_datasource,
    ) {
        value.sort(); // TODO: Use human friendly sorting
        value.dedup();
        let mut response = format!(
            "# Recursed route resolution for '{}' in {}μs, {} items\n",
            name,
            old_time.elapsed().as_micros(),
            value.len()
        );
        response.push_str(&serde_json::to_string_pretty(&value).unwrap());
        return response;
    } else {
        return format!("Value for '{}' not found in cache.", name);
    }
}

async fn get_asn_from_as_set(
    State(state): State<APIState>,
    Query(query): Query<QueryParameter>,
) -> impl IntoResponse {
    let old_time = Instant::now();
    let name = query.name.clone();
    let recursion_depth = query
        .recursion_depth
        .or(Some(state.config.default_recursion_depth))
        .unwrap();
    let ignore_as_sets = query
        .ignore_as_sets
        .clone()
        .split(',')
        .map(|a| a.to_string())
        .filter(|a| a != "")
        .collect::<Vec<String>>();
    let ignore_datasource = query
        .ignore_datasource
        .clone()
        .split(',')
        .map(|a| a.to_string())
        .filter(|a| a != "")
        .collect::<Vec<String>>();
    if let Some(mut value) = state.store.query_as_set_recursive(
        name.to_string(),
        recursion_depth,
        ignore_as_sets,
        &ignore_datasource,
    ) {
        value.sort(); // TODO: Use human friendly sorting
        value.dedup();
        let mut response = format!(
            "# Recursed AS-Set resolution for '{}' in {}μs, {} items\n",
            name,
            old_time.elapsed().as_micros(),
            value.len()
        );
        response.push_str(&serde_json::to_string_pretty(&value).unwrap());
        return response;
    } else {
        return format!("Value for '{}' not found in cache.", name);
    }
}

async fn get_as_set(
    State(state): State<APIState>,
    Query(query): Query<QueryParameter>,
) -> impl IntoResponse {
    let old_time = Instant::now();
    let name = query.name.clone();
    let ignore_datasource = query
        .ignore_datasource
        .clone()
        .split(',')
        .map(|a| a.to_string())
        .filter(|a| a != "")
        .collect::<Vec<String>>();
    let datasources = query
        .datasources
        .clone()
        .split(',')
        .map(|a| a.to_string())
        .filter(|a| a != "")
        .collect::<Vec<String>>();
    if let Some(mut value) =
        state
            .store
            .query_as_set(datasources, name.to_string(), &ignore_datasource)
    {
        let mut result = Vec::new();
        result.append(&mut value.as_sets);
        result.append(&mut value.asns);
        result.sort(); // TODO: Use human friendly sorting
        result.dedup();
        let mut response = format!(
            "# Requested AS-Set for '{}' in {}μs, {} items\n",
            name,
            old_time.elapsed().as_micros(),
            result.len()
        );
        response.push_str(&serde_json::to_string_pretty(&result).unwrap());
        return response;
    } else {
        return format!("AS-SET '{}' not found in cache.", name);
    }
}

pub async fn listen(config: ConfigAPI, store: Arc<store::DataStore>) {
    let router_v1 = Router::new()
        .route("/asSet", get(get_as_set))
        .route("/asnsFromAsSet", get(get_asn_from_as_set))
        .route("/routesFromAsSet", get(get_route_from_as_set))
        .with_state(APIState {
            config: config.clone(),
            store: store.clone(),
        });

    let mut router = Router::new();

    router = router.nest("/api/v1", router_v1);

    let make_service = router.into_make_service();

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(config.listen_address)
        .await
        .unwrap();
    axum::serve(listener, make_service).await.unwrap();
}
