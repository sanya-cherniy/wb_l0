#!/bin/bash

# Проверяем, что передан путь к файлу
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <path_to_file>"
    exit 1
fi

# Сохраняем аргумент в переменную
filename=$1

# Проверяем, существует ли файл
if [ ! -f "$filename" ]; then
    echo "File not found: $filename"
    exit 1
fi

# Выполняем запрос curl с файлом
curl -X POST http://localhost:8081/order \
     -H 'Content-Type: application/json' \
     -d @"$filename"
