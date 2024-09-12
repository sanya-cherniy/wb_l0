use crate::db_module::AppState;
use crate::db_module::Order;
use axum::routing::get;
use axum::{extract::Json, response::IntoResponse, routing::post, Router};
use dotenv::dotenv;
use sqlx::postgres::PgPool;
use sqlx::Error;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
mod db_module;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url).await?; // подключение к БД

    let tables = vec!["payment", "delivery", "orders", "item"]; // массив с названиями таблиц которые необходимо создать в БД

    // Проходим по каждому имени таблицы и проверяем её существование
    for table in tables {
        let exists_query = r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = 'public'
                AND table_name = $1
            );
        "#; // выполняем запрос для проверки существования таблицы
        let exists: (bool,) = sqlx::query_as(exists_query)
            .bind(table)
            .fetch_one(&pool)
            .await?;

        if !exists.0 {
            // если таблицы нет в базе данных создаем ее
            println!("the '{}' table does not exist, creating a table", table);
            db_module::create_table(table, &pool).await?;
        } else {
            println!("the '{}' table exists.", table);
        }
    }

    let app_state = Arc::new(Mutex::new(AppState::new())); // инициализация переменной которая хранит заказы

    let mut state = app_state.lock().unwrap();
    state
        .load_orders(&pool)
        .await
        .expect("Failed to load orders"); // загрузка заказов из базы данных, в том случае если они там есть

    // инициализация маршрутов
    let app = Router::new()
        .route(
            "/order",
            post({
                let pool = pool.clone();
                let app_state = app_state.clone();
                move |input: Json<db_module::Order>| state_handler(app_state, input, pool)
                // передаем пул для подключения к БД и данные заказов
            }),
        ) // post запрос на который отправляются заказы
        .route(
            "/orders",
            get({
                let app_state = app_state.clone();
                move || get_state(app_state)
            }), // get запрос который возвращает заказы
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 8081)); // указываем адрес сервера и порт

    // Создаем сервер на указанном адресе
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

// обработчик post запросов
async fn state_handler(
    state: Arc<Mutex<AppState>>,
    Json(payload): Json<db_module::Order>,
    _pool: PgPool, // извлекаем пул подключений
                   // извлекаем общее состояние
) -> impl IntoResponse {
    match insert_order(&_pool, &payload.clone()).await {
        Ok(true) => {
            let mut app_state = state.lock().unwrap();
            app_state.add_order(payload);
            (axum::http::StatusCode::OK, "Order received\n")
        }
        Ok(false) => {
            // данные уже содержатся в базе
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Order dont received: data already exists",
            );
        }
        Err(_) => {
            // ошибка вставки
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Order don't received: server error",
            );
        }
    }
}
async fn get_state(state: Arc<Mutex<AppState>>) -> impl IntoResponse {
    // Получаем доступ к mutex guard
    let locked_state = state.lock().unwrap();

    // Возвращаем текущее состояние в виде JSON
    Json(locked_state.get_orders()) // Здесь используется ссылка на locked_state
}

async fn insert_order(pool: &PgPool, order: &Order) -> Result<bool, Error> {
    // Проверяем, содержится ли в базе запись с указанным "order_uid"
    match check_order_exists(&pool, &order.order_uid).await {
        Ok(true) => Ok(false), // запись уже есть в БД
        Ok(false) => {
            // запись отстутствует, выполняем вставку
            let delivery_id: i32 = sqlx::query!(
                r#"
        INSERT INTO delivery (name, phone, zip, city, address, region, email)
        VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id
        "#,
                order.delivery.name,
                order.delivery.phone,
                order.delivery.zip,
                order.delivery.city,
                order.delivery.address,
                order.delivery.region,
                order.delivery.email,
            )
            .fetch_one(pool)
            .await?
            .id;

            let payment_id: i32 = sqlx::query!(
        r#"
        INSERT INTO payment (transaction, request_id, currency, provider, amount, payment_dt, bank, delivery_cost, goods_total, custom_fee)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id
        "#,
        order.payment.transaction,
        order.payment.request_id,
        order.payment.currency,
        order.payment.provider,
        order.payment.amount,
        order.payment.payment_dt,
        order.payment.bank,
        order.payment.delivery_cost,
        order.payment.goods_total,
        order.payment.custom_fee,
    )
    .fetch_one(pool)
    .await?
    .id;
            sqlx::query!(
        r#"
        INSERT INTO orders (order_uid, track_number, entry, delivery_id, payment_id, locale, internal_signature, customer_id, delivery_service, shardkey, sm_id, date_created, oof_shard)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
        order.order_uid,
        order.track_number,
        order.entry,
        delivery_id,
        payment_id,
        order.locale,
        order.internal_signature,
        order.customer_id,
        order.delivery_service,
        order.shardkey,
        order.sm_id,
        order.date_created,
        order.oof_shard,
    )
    .execute(pool)
    .await?;
            for item in &order.items {
                sqlx::query!(
            r#"
            INSERT INTO item (chrt_id, track_number, price, rid, name, sale, size, total_price, nm_id, brand, status, order_uid)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
            item.chrt_id,
            item.track_number,
            item.price,
            item.rid,
            item.name,
            item.sale,
            item.size,
            item.total_price,
            item.nm_id,
            item.brand,
            item.status,
            order.order_uid,
        )
        .execute(pool)
        .await?;
            }
            Ok(true)
        }
        Err(err) => Err(err),
    }
}

// Функция для проверки полученных данных на то что они уже есть БД
async fn check_order_exists(db_pool: &PgPool, uid: &String) -> Result<bool, Error> {
    // Выполняем запрос для поиска записи в БД по значению "order_uid"
    let result = sqlx::query!(
        r#"
        SELECT order_uid FROM orders WHERE order_uid = $1
        "#,
        uid
    )
    .fetch_optional(db_pool)
    .await;

    match result {
        Ok(Some(_)) => Ok(true), // запись найдена
        Ok(None) => Ok(false),   // запись не найдена
        Err(err) => Err(err),    // обработка ошибки
    }
}

// Тесты
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use reqwest::Client;
    use sqlx::PgPool;
    use std::env;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tokio;

    // Функция инициализации БД
    async fn setup_database() -> PgPool {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = PgPool::connect(&database_url).await.unwrap();

        // Очистка и повторное создание таблиц
        let drop_item_table = r#"DROP TABLE IF EXISTS item CASCADE;"#;
        let drop_orders_table = r#"DROP TABLE IF EXISTS orders CASCADE;"#;
        let drop_payment_table = r#"DROP TABLE IF EXISTS payment CASCADE;"#;
        let drop_delivery_table = r#"DROP TABLE IF EXISTS delivery CASCADE;"#;

        sqlx::query(drop_item_table).execute(&pool).await.unwrap();
        sqlx::query(drop_orders_table).execute(&pool).await.unwrap();
        sqlx::query(drop_payment_table)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(drop_delivery_table)
            .execute(&pool)
            .await
            .unwrap();

        // Создание таблиц
        let tables = vec!["payment", "delivery", "orders", "item"];
        for table in tables {
            db_module::create_table(table, &pool).await.unwrap();
        }

        pool
    }

    async fn load_json_from_file(file_path: &str) -> serde_json::Value {
        let content = fs::read_to_string(file_path).expect("Unable to read file");
        serde_json::from_str(&content).expect("JSON was not well-formatted")
    }

    // Функция генерирующая post запрос с указанным json
    async fn perform_test_order_request(order_data: &serde_json::Value) -> StatusCode {
        let client = Client::new();
        let response = client
            .post("http://127.0.0.1:8081/order")
            .json(order_data)
            .send()
            .await
            .unwrap();

        response.status() // возвращаем код ответа
    }

    #[tokio::test]
    async fn test_order_creation() {
        let pool = setup_database().await;

        // Запускаем сервер в фоновом режиме
        let app_state = Arc::new(Mutex::new(AppState::new()));
        {
            let mut state = app_state.lock().unwrap();
            state.load_orders(&pool).await.unwrap();
        }

        let app = Router::new()
            .route(
                "/order",
                post({
                    let pool = pool.clone();
                    let app_state = app_state.clone();
                    move |input: Json<db_module::Order>| state_handler(app_state, input, pool)
                }),
            )
            .route(
                "/orders",
                get({
                    let app_state = app_state.clone();
                    move || get_state(app_state)
                }),
            );

        let addr = SocketAddr::from(([127, 0, 0, 1], 8081));

        tokio::spawn(axum::Server::bind(&addr).serve(app.into_make_service()));

        // Различные валидные данные
        let json_data_1 = load_json_from_file("models/model1.json").await;
        let json_data_2 = load_json_from_file("models/model2.json").await;
        let json_data_3 = load_json_from_file("models/model3.json").await;
        let json_data_4 = load_json_from_file("models/model4.json").await;
        let json_data_5 = load_json_from_file("models/model5.json").await;
        let json_data_6 = load_json_from_file("models/model_extended.json").await;
        // Невалидные данные
        let json_data_incorrect_1 = load_json_from_file("models/model_not_correct.json").await;
        let json_data_incorrect_2 = load_json_from_file("models/model_empty.json").await;
        let json_data_incorrect_3 = load_json_from_file("models/model1.json").await; // данные которые уже есть в БД

        // Выполняем post запросы
        let status_1 = perform_test_order_request(&json_data_1).await;
        let status_2 = perform_test_order_request(&json_data_2).await;
        let status_3 = perform_test_order_request(&json_data_3).await;
        let status_4 = perform_test_order_request(&json_data_4).await;
        let status_5 = perform_test_order_request(&json_data_5).await;
        let status_6 = perform_test_order_request(&json_data_6).await;

        let status_7 = perform_test_order_request(&json_data_incorrect_2).await;
        let status_8 = perform_test_order_request(&json_data_incorrect_1).await;
        let status_9 = perform_test_order_request(&json_data_incorrect_3).await;

        assert_eq!(status_1, StatusCode::OK);
        assert_eq!(status_2, StatusCode::OK);
        assert_eq!(status_3, StatusCode::OK);
        assert_eq!(status_4, StatusCode::OK);
        assert_eq!(status_5, StatusCode::OK);
        assert_eq!(status_6, StatusCode::OK);

        assert_eq!(status_7, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(status_8, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(status_9, StatusCode::INTERNAL_SERVER_ERROR);

        // Проверка получения всех заказов
        let client = Client::new();
        let orders_response = client
            .get("http://127.0.0.1:8081/orders")
            .send()
            .await
            .unwrap();

        assert_eq!(orders_response.status(), StatusCode::OK);
        let orders_json: serde_json::Value = orders_response.json().await.unwrap();
        assert!(orders_json.is_array()); // проверяем, что ответ - массив
        assert!(orders_json.as_array().unwrap().len() > 0); // проверяем, что есть хотя бы один заказ

        // Проверка соответствия отправленных и полученных данных
        assert_eq!(json_data_1, orders_json[0]);
        assert_eq!(json_data_2, orders_json[1]);
        assert_eq!(json_data_3, orders_json[2]);
        assert_eq!(json_data_4, orders_json[3]);
        assert_eq!(json_data_5, orders_json[4]);
        assert_eq!(json_data_6, orders_json[5]);
    }
}
