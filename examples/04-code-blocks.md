# Code Blocks

## Indented Code Block

    This is a code block
    created with indentation
    (4 spaces or 1 tab)

## Fenced Code Blocks

```
Plain fenced code block
without language specification
```

### Rust

```rust
fn main() {
    let greeting = "Hello, World!";
    println!("{}", greeting);

    let numbers: Vec<i32> = (1..=10).collect();
    let sum: i32 = numbers.iter().sum();
    println!("Sum: {}", sum);
}

#[derive(Debug)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
```

### Python

```python
def fibonacci(n: int) -> list[int]:
    """Generate Fibonacci sequence up to n terms."""
    if n <= 0:
        return []
    elif n == 1:
        return [0]

    sequence = [0, 1]
    while len(sequence) < n:
        sequence.append(sequence[-1] + sequence[-2])
    return sequence

# Usage
fib = fibonacci(10)
print(f"First 10 Fibonacci numbers: {fib}")

class Calculator:
    def __init__(self):
        self.result = 0

    def add(self, value):
        self.result += value
        return self
```

### JavaScript

```javascript
const fetchData = async (url) => {
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const data = await response.json();
    return data;
  } catch (error) {
    console.error('Fetch error:', error);
    throw error;
  }
};

class EventEmitter {
  constructor() {
    this.events = {};
  }

  on(event, callback) {
    if (!this.events[event]) {
      this.events[event] = [];
    }
    this.events[event].push(callback);
  }

  emit(event, ...args) {
    if (this.events[event]) {
      this.events[event].forEach(cb => cb(...args));
    }
  }
}
```

### Go

```go
package main

import (
    "fmt"
    "sync"
)

func worker(id int, jobs <-chan int, results chan<- int, wg *sync.WaitGroup) {
    defer wg.Done()
    for job := range jobs {
        fmt.Printf("Worker %d processing job %d\n", id, job)
        results <- job * 2
    }
}

func main() {
    jobs := make(chan int, 100)
    results := make(chan int, 100)
    var wg sync.WaitGroup

    for w := 1; w <= 3; w++ {
        wg.Add(1)
        go worker(w, jobs, results, &wg)
    }

    for j := 1; j <= 9; j++ {
        jobs <- j
    }
    close(jobs)

    wg.Wait()
    close(results)
}
```

### Shell/Bash

```bash
#!/bin/bash

# Script to backup files
BACKUP_DIR="/backup/$(date +%Y%m%d)"
SOURCE_DIR="/data"

mkdir -p "$BACKUP_DIR"

for file in "$SOURCE_DIR"/*; do
    if [ -f "$file" ]; then
        cp "$file" "$BACKUP_DIR/"
        echo "Backed up: $file"
    fi
done

echo "Backup complete: $(ls -1 "$BACKUP_DIR" | wc -l) files"
```

### SQL

```sql
SELECT
    u.id,
    u.username,
    COUNT(o.id) AS order_count,
    SUM(o.total) AS total_spent
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE u.created_at >= '2024-01-01'
GROUP BY u.id, u.username
HAVING COUNT(o.id) > 5
ORDER BY total_spent DESC
LIMIT 10;
```

### JSON

```json
{
  "name": "gander",
  "version": "0.1.0",
  "dependencies": {
    "ratatui": "0.30",
    "comrak": "0.31",
    "syntect": "5"
  },
  "features": {
    "images": true,
    "syntax_highlighting": true,
    "file_watching": true
  }
}
```

### YAML

```yaml
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
    volumes:
      - ./html:/usr/share/nginx/html:ro
    environment:
      - NGINX_HOST=example.com
      - NGINX_PORT=80
    depends_on:
      - api

  api:
    build: ./api
    ports:
      - "3000:3000"
    env_file:
      - .env
```

### TOML

```toml
[package]
name = "gander"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.30"
comrak = "0.31"
syntect = "5"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
insta = "1"
proptest = "1"
```
