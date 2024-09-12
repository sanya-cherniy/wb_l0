use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use sqlx::FromRow;

// Запросы для создания таблиц в БД
pub static CREATE_DELIVERY_TABLE: &str = r#"
        CREATE TABLE delivery (
            id SERIAL PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            phone VARCHAR(50) NOT NULL,
            zip VARCHAR(20) NOT NULL,
            city VARCHAR(100) NOT NULL,
            address VARCHAR(255) NOT NULL,
            region VARCHAR(100) NOT NULL,
            email VARCHAR(100) NOT NULL
        );
    "#;

pub static CREATE_PAYMENT_TABLE: &str = r#"
        CREATE TABLE payment (
            id SERIAL PRIMARY KEY,
            transaction VARCHAR(255) NOT NULL,
            request_id VARCHAR(255) NOT NULL,
            currency VARCHAR(10) NOT NULL,
            provider VARCHAR(100) NOT NULL,
            amount INTEGER NOT NULL,
            payment_dt BIGINT NOT NULL,
            bank VARCHAR(100) NOT NULL,
            delivery_cost INTEGER NOT NULL,
            goods_total INTEGER NOT NULL,
            custom_fee INTEGER NOT NULL
        );
    "#;

pub static CREATE_ORDERS_TABLE: &str = r#"
        CREATE TABLE orders (
            order_uid VARCHAR(255) PRIMARY KEY,
            track_number VARCHAR(255) NOT NULL,
            entry VARCHAR(255) NOT NULL,
            delivery_id INTEGER REFERENCES delivery(id) ON DELETE CASCADE,
            payment_id INTEGER REFERENCES payment(id) ON DELETE CASCADE,
            locale VARCHAR(10) NOT NULL,
            internal_signature VARCHAR(255) NOT NULL,
            customer_id VARCHAR(255) NOT NULL,
            delivery_service VARCHAR(100) NOT NULL,
            shardkey VARCHAR(50) NOT NULL,
            sm_id INTEGER NOT NULL,
            date_created VARCHAR(50) NOT NULL,
            oof_shard VARCHAR(50) NOT NULL
        );
    "#;

pub static CREATE_ITEM_TABLE: &str = r#"
        CREATE TABLE item (
            id SERIAL PRIMARY KEY,
            chrt_id INTEGER NOT NULL,
            track_number VARCHAR(255) NOT NULL,
            price INTEGER NOT NULL,
            rid VARCHAR(255) NOT NULL,
            name VARCHAR(255) NOT NULL,
            sale INTEGER NOT NULL,
            size VARCHAR(50) NOT NULL,
            total_price INTEGER NOT NULL,
            nm_id INTEGER NOT NULL,
            brand VARCHAR(100) NOT NULL,
            status INTEGER NOT NULL,
            order_uid VARCHAR(255) REFERENCES orders(order_uid) ON DELETE CASCADE
        );
    "#;

// Структуры для хранения заказов
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub order_uid: String,
    pub track_number: String,
    pub entry: String,
    pub delivery: Delivery,
    pub payment: Payment,
    pub items: Vec<Item>,
    pub locale: String,
    pub internal_signature: String,
    pub customer_id: String,
    pub delivery_service: String,
    pub shardkey: String,
    pub sm_id: i32,
    pub date_created: String,
    pub oof_shard: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct Delivery {
    pub name: String,
    pub phone: String,
    pub zip: String,
    pub city: String,
    pub address: String,
    pub region: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct Payment {
    pub transaction: String,
    pub request_id: String,
    pub currency: String,
    pub provider: String,
    pub amount: i32,
    pub payment_dt: i64,
    pub bank: String,
    pub delivery_cost: i32,
    pub goods_total: i32,
    pub custom_fee: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct Item {
    pub chrt_id: i32,
    pub track_number: String,
    pub price: i32,
    pub rid: String,
    pub name: String,
    pub sale: i32,
    pub size: String,
    pub total_price: i32,
    pub nm_id: i32,
    pub brand: String,
    pub status: i32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppState {
    orders: Vec<Order>, // Здесь мы храним заказы
}

impl AppState {
    // Создание нового состояния с пустым списком заказов
    pub fn new() -> Self {
        AppState { orders: Vec::new() }
    }
    // Добавление нового заказа
    pub fn add_order(&mut self, order: Order) {
        self.orders.push(order);
    }
    // Получение заказов
    pub fn get_orders(&self) -> Vec<Order> {
        self.orders.clone() // Клонируем заказы для возврата
    }
}

// Структура для загрузки данных из базы в AppState
#[derive(FromRow, Debug)]
struct OrderRow {
    order_uid: String,
    track_number: String,
    entry: String,
    delivery_id: i32,
    payment_id: i32,
    locale: String,
    internal_signature: String,
    customer_id: String,
    delivery_service: String,
    shardkey: String,
    sm_id: i32,
    date_created: String,
    oof_shard: String,
}

// Метод для загрузки данных из базы в AppState
impl AppState {
    pub async fn load_orders(&mut self, db_pool: &PgPool) -> Result<(), sqlx::Error> {
        // Загружаем заказы
        let orders: Vec<OrderRow> = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders")
            .fetch_all(db_pool)
            .await?;

        for order_row in orders {
            // Загружаем соответствующий delivery
            let delivery_row: Delivery = sqlx::query_as("SELECT * FROM delivery WHERE id = $1")
                .bind(order_row.delivery_id)
                .fetch_one(db_pool)
                .await?;

            // Загружаем соответствующий payment
            let payment_row: Payment = sqlx::query_as("SELECT * FROM payment WHERE id = $1")
                .bind(order_row.payment_id)
                .fetch_one(db_pool)
                .await?;

            // Загружаем соответствующие items
            let items: Vec<Item> =
                sqlx::query_as::<_, Item>("SELECT * FROM item WHERE order_uid = $1")
                    .bind(order_row.order_uid.clone())
                    .fetch_all(db_pool)
                    .await?;

            // Преобразуем загруженные данные в нужные структуры
            let delivery = Delivery {
                name: delivery_row.name,
                phone: delivery_row.phone,
                zip: delivery_row.zip,
                city: delivery_row.city,
                address: delivery_row.address,
                region: delivery_row.region,
                email: delivery_row.email,
            };

            let payment = Payment {
                transaction: payment_row.transaction,
                request_id: payment_row.request_id,
                currency: payment_row.currency,
                provider: payment_row.provider,
                amount: payment_row.amount,
                payment_dt: payment_row.payment_dt,
                bank: payment_row.bank,
                delivery_cost: payment_row.delivery_cost,
                goods_total: payment_row.goods_total,
                custom_fee: payment_row.custom_fee,
            };

            let items_vec: Vec<Item> = items
                .iter()
                .map(|item_row| Item {
                    chrt_id: item_row.chrt_id,
                    track_number: item_row.track_number.clone(),
                    price: item_row.price,
                    rid: item_row.rid.clone(),
                    name: item_row.name.clone(),
                    sale: item_row.sale,
                    size: item_row.size.clone(),
                    total_price: item_row.total_price,
                    nm_id: item_row.nm_id,
                    brand: item_row.brand.clone(),
                    status: item_row.status,
                })
                .collect();

            // Создаем и добавляем новый заказ в AppState
            let order = Order {
                order_uid: order_row.order_uid,
                track_number: order_row.track_number,
                entry: order_row.entry,
                delivery, // Используем уже созданную структуру Delivery
                payment,  // Используем уже созданную структуру Payment
                items: items_vec,
                locale: order_row.locale,
                internal_signature: order_row.internal_signature,
                customer_id: order_row.customer_id,
                delivery_service: order_row.delivery_service,
                shardkey: order_row.shardkey,
                sm_id: order_row.sm_id,
                date_created: order_row.date_created,
                oof_shard: order_row.oof_shard,
            };

            self.add_order(order); // Добавляем в AppState
        }

        Ok(())
    }
}
// Функция для создания таблиц
pub async fn create_table(table: &str, pool: &PgPool) -> Result<(), sqlx::Error> {
    match table {
        "payment" => {
            sqlx::query(CREATE_PAYMENT_TABLE).execute(pool).await?;
            Ok(())
        }
        "delivery" => {
            sqlx::query(CREATE_DELIVERY_TABLE).execute(pool).await?;
            Ok(())
        }
        "orders" => {
            sqlx::query(CREATE_ORDERS_TABLE).execute(pool).await?;
            Ok(())
        }
        "item" => {
            sqlx::query(CREATE_ITEM_TABLE).execute(pool).await?;
            Ok(())
        }
        _ => {
            return Err(sqlx::Error::Protocol(table.to_string()));
        }
    }
}
