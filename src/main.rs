mod handlers;
mod repositories;

use crate::handlers::{all_todo, create_todo, delete_todo, find_todo, update_todo};
use crate::repositories::{TodoRepository, TodoRepositoryForMemory};

use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    extract::Extension,
    routing::{get, post},
    Router,
};

#[tokio::main]
async fn main() {
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();

    let repository = TodoRepositoryForMemory::new();
    let app = create_app(repository);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    tracing::debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn create_app<T: TodoRepository>(repository: T) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/todos", post(create_todo::<T>).get(all_todo::<T>))
        .route(
            "/todos/:id",
            get(find_todo::<T>)
                .delete(delete_todo::<T>)
                .patch(update_todo::<T>),
        )
        .layer(Extension(Arc::new(repository)))
}

async fn root() -> &'static str {
    "Hello, world!"
}

#[cfg(test)]
mod test {
    use crate::repositories::{CreateTodo, Todo};

    use super::*;
    use axum::{body::Body, response::Response};
    use hyper::{header, Method, Request, StatusCode};
    use tower::ServiceExt;

    fn build_todo_req_with_json(path: &str, method: Method, json_body: String) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(json_body))
            .unwrap()
    }

    fn build_todo_req_with_empty(path: &str, method: Method) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::empty())
            .unwrap()
    }

    async fn res_to_todo(res: Response) -> Todo {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: Todo = serde_json::from_str(&body)
            .expect(&format!("cannnot convert Todo instance. body: {}", body));
        todo
    }

    #[tokio::test]
    async fn should_return_hello_world() {
        let repository = TodoRepositoryForMemory::new();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = create_app(repository).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "Hello, world!")
    }

    #[tokio::test]
    async fn should_created_todo() {
        let expected = Todo::new(1, "should_return_created_todo".to_string());
        let repository = TodoRepositoryForMemory::new();
        let req = build_todo_req_with_json(
            "/todos",
            Method::POST,
            r#"{ "text" : "should_return_created_todo" }"#.to_string(),
        );

        let res = create_app(repository).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_find_todo() {
        let expexted = Todo::new(1, "should_find_todo".to_string());
        let repository = TodoRepositoryForMemory::new();
        repository.create(CreateTodo::new("should_find_todo".to_string()));
        let req = build_todo_req_with_empty("/todos/1", Method::GET);

        let res = create_app(repository).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expexted, todo);
    }

    #[tokio::test]
    async fn should_get_all_todos() {
        let expected = Todo::new(1, "should_get_all_todos".to_string());
        let repository = TodoRepositoryForMemory::new();
        repository.create(CreateTodo::new("should_get_all_todos".to_string()));
        let req = build_todo_req_with_empty("/todos", Method::GET);

        let res = create_app(repository).oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: Vec<Todo> = serde_json::from_str(&body)
            .expect(&format!("cannnot convert Todo inscance. body: {}", body));

        assert_eq!(vec![expected], todo);
    }

    #[tokio::test]
    async fn should_update_todo() {
        let expected = Todo::new(1, "after_update_todo".to_string());
        let repository = TodoRepositoryForMemory::new();
        repository.create(CreateTodo::new("before_update_todo".to_string()));

        let req = build_todo_req_with_json(
            "/todos/1",
            Method::PATCH,
            r#"{ "id": 1, "text" : "after_update_todo" }"#.to_string(),
        );

        let res = create_app(repository).oneshot(req).await.unwrap();
        let todo = res_to_todo(res).await;

        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_delete_todo() {
        let repository = TodoRepositoryForMemory::new();
        repository.create(CreateTodo::new("before_delete_todo".to_string()));

        let req = build_todo_req_with_empty("/todos/1", Method::DELETE);

        let res = create_app(repository).oneshot(req).await.unwrap();

        assert_eq!(StatusCode::NO_CONTENT, res.status());
    }

    #[tokio::test]
    async fn should_fail_validate_empty_text() {
        let repository = TodoRepositoryForMemory::new();

        let req =
            build_todo_req_with_json("/todos", Method::POST, r#"{ "text" : "" }"#.to_string());

        let res = create_app(repository).oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(
            "Validation error: [text: Can not be empty]".to_string(),
            body
        );
    }

    #[tokio::test]
    async fn should_fail_validate_over_100_text() {
        let repository = TodoRepositoryForMemory::new();

        let req = build_todo_req_with_json(
            "/todos",
            Method::POST,
            r#"{ "text" : "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }"#.to_string()
        );

        let res = create_app(repository).oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(
            "Validation error: [text: Over text length]".to_string(),
            body
        );
    }
}
