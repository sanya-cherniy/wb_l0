# wb_l0

# Курс "Быстрый Rust"
## L0
Представляю вашему вниманию мое решение задания L0 курса "Быстрый Rust"

Запуск сервера:
```sh
cargo run
```
Запуск тестов:
```sh
cargo test
```
Сервер работает на порту
```sh
127.0.0.1:8081
```
В файле ".env" указан URL для подключения к базе данных

В директории "models" расположены json-файлы для тестирования проекта

При помощи скрипта "json_upload.sh" можно загрузить json на сервер

```sh
./json_updload.sh [FILE]
```

## Запуск базы данных в Docker

```sh
docker run --name postgres_l0 -p 5432:5432 -e POSTGRES_PASSWORD=postgres -d postgres
```
