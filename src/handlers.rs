use crate::repositories::{CreateTodo, TodoRepository};
use axum::{extract::Extension, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;

pub async fn create_todo<T: TodoRepository>(
    Json(payload): Json<CreateTodo>,
    Extension(repository): Extension<Arc<T>>,
) -> impl IntoResponse {
    let todo = repository.create(payload);

    (StatusCode::CREATED, Json(todo))
}
