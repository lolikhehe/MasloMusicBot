# Контекст сесії розробки Telegram-бота для музики на Rust

**Шлях до проекту:** `C:\Users\pylyp\music-bot`  
**Гілка Git:** `main`  
**Дата вилучення з бази даних OpenCode:** 16 листопада 2026 р.

---

## 1. Загальна мета проекту
Створення високопродуктивного Telegram-бота для завантаження аудіо/музики на Rust (tokio, teloxide, sqlite, yt-dlp, ffmpeg) відповідно до ТЗ.

---

## 2. Що вже повністю реалізовано
- **Ядро:** Tokio runtime, teloxide 0.17 для Telegram Bot API, mimalloc алокатор.
- **База даних і кеш:** SQLite в режимі WAL через `sqlx 0.9` (`/data/bot.db`). Кешуються тільки метадані та Telegram `file_id`/`file_unique_id` (самі аудіофайли не зберігаються локально надовго).
- **Провайдери та парсери URL:**
  - YouTube, YouTube Music, SoundCloud, Bandcamp, Apple Music, Deezer.
  - Spotify (окремі треки через публічний oEmbed + пошук на YouTube; альбоми/плейлісти відхиляються згідно з ТЗ без авторизації).
- **Завантажувач (Download Manager):**
  - Виклики `yt-dlp` та `ffmpeg` як підпроцесів (через масиви аргументів, без шеллу, з жорсткими таймаутами).
  - Дедуплікація одночасних однакових запитів через `tokio::sync::broadcast`.
  - Обмеження паралельних завантажень через семафор.
  - Тегування ID3/MP4 через `lofty 0.25`, оптимізація та масштабування обкладинок через `ffmpeg`.
- **Функціонал бота:**
  - Команди: `/start`, `/help`, `/settings` (мова UK/EN).
  - Адмін-команди: `/stats`, `/ban`, `/unban`, `/purge`.
  - Inline mode з підтримкою кешу та кнопок переходу.
  - Rate limiting по користувачах.
  - Prometheus-метрики на порті `9090` (`metrics-exporter-prometheus 0.18`).
- **Тестування та збірка:**
  - `cargo check --all-targets` — 0 помилок, 0 попереджень.
  - `cargo test` — 25/25 тестів проходять успішно.
  - `cargo build --release --locked` — успішно компілюється.
  - Налаштовано `Dockerfile` (multi-stage non-root) та `docker-compose.yml`.

---

## 3. Останні критичні зміни та поточний стан (на момент зупинки)
1. **Виявлена та виправлена проблема з `yt-dlp` в Docker:**
   - PyInstaller (з якого зібраний `yt-dlp`) розпаковує бінарні залежності у тимчасову папку і вимагає права на виконання (`mmap`/`exec`).
   - У Docker контейнері `/tmp` монтувався як tmpfs з опцією `noexec`, через що `yt-dlp` падав під час спроби завантаження треків.
   - **Виправлення:** додано `TMPDIR=/data/tmp` у `docker-compose.yml` та `Dockerfile`.
2. **Стан Docker контейнера:**
   - Контейнер `music-bot-bot-1` перестворено та запущено. Статус: `Up (healthy)`.
   - Бот успішно з'єднався з Telegram під іменем `@MaslougazeMusicBot`.
   - `yt-dlp --version` всередині контейнера повертає коректну версію `2026.10.26`.
3. **Зміни у Git (робоче дерево):**
   - Модифіковано `Dockerfile` та `docker-compose.yml` (додано `TMPDIR=/data/tmp`).

---

## 4. Що залишилося зробити (Next Steps для нової сесії)
1. **Оновити документацію:**
   - `docs/decisions.md`: зафіксувати рішення щодо `TMPDIR=/data/tmp` для PyInstaller/noexec.
   - `docs/perf.md`: додати заміри та статус верифікації розгортання.
2. **Перевірка роботи бота на реальних треках:**
   - Надіслати боту в Telegram тестове посилання на трек YouTube або SoundCloud.
   - Переконатися, що аудіо завантажується, обкладинка/теги на місці, а повторний запит береться з кешу (`file_id`).
3. **Фіксація в Git:**
   - Закомітити зміни в `Dockerfile`, `docker-compose.yml` та оновлену документацію:
     ```bash
     git add Dockerfile docker-compose.yml docs/
     git commit -m "fix(docker): set TMPDIR to /data/tmp for yt-dlp PyInstaller execution"
     ```
