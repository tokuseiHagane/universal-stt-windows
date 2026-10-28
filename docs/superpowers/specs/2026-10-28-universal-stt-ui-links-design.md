# Universal STT — стабилизация проекта и импорт ссылок

Дата: 2026-10-28
Статус: ожидает утверждения пользователя
Связанный документ: `2026-10-27-universal-stt-windows-design.md`

## 1. Намерение

Довести уже работающий импорт до пригодного повседневного редактора: после обработки MP4 пользователь должен видеть видео и транскрипт на всю рабочую высоту, играть и перематывать запись, переходить между дорожкой спикеров и текстом, переименовывать спикеров. На стартовом экране должны работать как локальные файлы, так и ссылки на direct media, YouTube, Yandex Video и VK.

Это уточнение существующего продукта, не визуальный редизайн и не смена стека. Релиз не публикуется до отдельного решения пользователя.

## 2. Подтверждённые исходные проблемы

1. `.project` объявляет три grid-строки, но содержит четыре дочерних блока. Панель спикеров получает `minmax(0, 1fr)`, а `.project-body` попадает в `auto`. В live Chromium: speaker bar 144 px, project body 360 px.
2. Нативный `ustt://` handler формирует `Response<Vec<u8>>`. Для `Range: bytes=0-` и обычного GET он читает весь proxy в память. Маленький web-demo играет, реальный VP9/Opus proxy 110,6 МиБ может не открыться и не перематываться.
3. Speaker turns рисуются полупрозрачно поверх waveform, отдельной дорожки спикеров и перехода к реплике нет.
4. В Processing показывается один overall ETA, ETA текущей фазы не вычисляется.
5. Welcome читает только `FileData::path()`; текст ссылки из `DataTransfer` не используется.
6. Выбор языка уже есть; сохранение языка исправлено в текущем незакоммиченном baseline.

В web-demo playback, seek и rename работают. Не переписывать эти компоненты без нового воспроизведения: нативные проблемы прежде всего связаны с layout и media transport.

## 3. Goals

- Исправить рабочую раскладку audio/video без чёрных пустот и сжатого транскрипта.
- Обеспечить нативный streaming playback с корректными Range/HEAD/206/416 ответами.
- Синхронизировать video/audio, timeline, playhead и активную реплику.
- Добавить отдельную speaker lane: цвет соответствует спикеру, клик по turn делает seek и прокрутку к реплике.
- Сохранить работающие rename, undo, автосохранение и ручной scroll; закрыть регрессионными тестами.
- Показывать одновременно ETA текущей фазы и общий ETA.
- Принимать один локальный файл или один URL через drop, paste и явное поле ввода.
- Скачивать direct media, YouTube, Yandex Video и VK через встроенные portable tools.
- Сохранить самодостаточный Windows ZIP без установки Python, Node или системного yt-dlp.

## 4. Non-goals / anti-goals

- Не менять Dioxus, дизайн-токены, Inter и общую dark/light тему.
- Не делать очередь, пакетную загрузку, playlists или одновременную обработку нескольких ссылок.
- Не заменять `<video>` на покадровый canvas-плеер.
- Не возвращать truncated body в ответ на `bytes=0-`.
- Не требовать сетевую докачку зависимостей после распаковки полного portable ZIP.
- Не обещать обход авторизации, DRM, приватных видео или антибот-защиты.
- Не публиковать release автоматически после реализации.

## 5. Approaches

### A — выбран пользователем

Одна рабочая итерация из двух этапов без релиза: сначала стабилизация UI/media, затем импорт ссылок и полное E2E. Позволяет проверить фундамент до добавления downloader, но вся работа остаётся в одном согласованном baseline.

### B — только стабилизация

Отложить ссылки. Быстрее, но не закрывает явный запрос пользователя.

### C — немедленный v0.1.5

Сначала выпустить уже имеющиеся runtime fixes. Пользователь отложил этот вариант.

## 6. Этап 1 — рабочая область и playback

### 6.1 Layout

`.project` получает четыре явные строки: `auto auto minmax(0, 1fr) auto`.

- Toolbar и SpeakerBar занимают только нужную высоту.
- `.project-body` получает всю оставшуюся высоту и `min-height: 0`.
- Transcript и preview имеют `min-width: 0; min-height: 0`; transcript scrolls внутри себя.
- Видео сохраняет aspect ratio через `object-fit: contain`; чёрный допускается только как letterbox внутри самого video, не как половина рабочей области.
- При ширине 1024 px не должно быть горизонтального overflow; audio/video переключаются по существующему `MediaKind`.

### 6.2 Native media transport

Рассмотрены два варианта:

- Оставить custom protocol: отклонено, Dioxus/wry API здесь требует готовый byte-vector body и не даёт настоящего streaming.
- **Выбрано:** небольшой loopback HTTP media server, запускаемый приложением на `127.0.0.1` со случайным свободным портом.

Требования:

- Только localhost, только зарегистрированные media tokens, canonical path остаётся внутри project root.
- GET/HEAD, один byte range, suffix/open-ended ranges, 200/206/404/416, корректные `Content-Length`, `Content-Range`, `Accept-Ranges`, MIME.
- Тело отдаётся файловым reader ограниченной длины; requested range не материализуется целиком в `Vec`.
- Отсутствует wildcard CORS и directory listing. URL содержит только существующий token, не путь к файлу.
- Web/wasm продолжает использовать mock assets; native protocol fallback не должен silently возвращать полный файл при ошибке сервера.
- Имя proxy файла не фиксируется: допустимы текущие mp4/m4a/webm/aac.

### 6.3 Player и синхронизация

- Сохранить единственный media element `#player` и существующий bridge.
- Play/pause, click seek, wheel seek, currentTime, duration, ended и playhead используют один `Playback` state.
- При project switch: stop, seek 0, новый duration, новый source, новый bridge generation.
- При отсутствии proxy показывать существующий понятный empty state, а не битый player.
- Ошибки media element должны давать видимое сообщение с MIME/именем proxy и возможностью повторно открыть проект; технические details остаются в логе.
- Плавный autoscroll работает во время playback. Ручной wheel отключает его до нажатия «текущая реплика» или явного перехода из timeline.

### 6.4 Timeline и transcript

- Под waveform добавляется отдельная speaker lane высотой примерно 24–28 px с доступным label/legend.
- Turn имеет цвет спикера, достаточный контраст и не полагается только на цвет: tooltip содержит имя и time range.
- Клик по lane определяет turn, делает seek к его началу и отправляет transcript scroll request к сегменту с максимальным перекрытием; tie выбирает более ранний сегмент.
- Клик по waveform сохраняет обычный seek.
- Seek на pause также обновляет активную реплику и прокручивает её в видимую область после явного перехода по turn/transcript.
- Виртуализация и редактирование текста сохраняются. Long-project test обязателен: >50 сегментов.
- Speaker rename остаётся inline input с commit на Enter/blur, отменой на Escape и записью в undo history. Сначала проверить после layout fix; не менять работающую модель без причины.

### 6.5 Processing ETA

`Progress` дополняется `phase_eta_seconds`, существующий `eta_seconds` остаётся overall.

- Phase ETA: elapsed внутри текущей фазы и её local fraction.
- Overall ETA: существующий monotonic overall fraction.
- При fraction 0, завершении или неизвестной работе допускается `—`; после первого ненулевого progress должна появляться оценка.
- UI выводит оба значения отдельными подписями RU/EN, без переполнения узкого окна.

## 7. Этап 2 — импорт ссылок

### 7.1 Ввод

Welcome принимает:

- drop локального файла — существующий native path;
- drop текста/URI — `event.data_transfer().get_as_text()`, затем `text/uri-list` fallback;
- paste и ручной ввод в поле URL + кнопку импорта.

Разрешены только `http` и `https`, один источник за раз. Пробелы и окружающие кавычки удаляются. Некорректный ввод показывает localized ошибку и не запускает worker. Поле ввода остаётся доступным, даже если конкретный Windows WebView2 не передаёт text DnD.

### 7.2 Downloader

Новый независимый модуль/крейт использует `yt-dlp.exe`, для YouTube — bundled `deno.exe` через явный `--js-runtimes deno:<path>`.

- Direct URLs обрабатывает generic extractor yt-dlp.
- YouTube, VK/VK Video, Yandex Video/Dzen используют их built-in extractors.
- `--no-playlist`: playlist URL не превращается в серию файлов; пользователь получает один выбранный элемент либо понятную ошибку.
- Выбор формата ограничивает preview/source разумным максимумом 1080p, но сохраняет лучший доступный аудиопоток. Последующий decode/proxy остаётся в существующем FFmpeg pipeline.
- Downloader сообщает final filepath и title; не полагается на угадывание расширения.
- Новый Download phase показывает downloaded bytes/percent, когда extractor их даёт, и indeterminate state иначе.
- Cancel завершает downloader, удаляет partial output и не оставляет проект.
- Ошибки сети, unsupported URL, login/DRM и отсутствующие tools различаются в localized сообщении. Подробный stderr сохраняется для диагностики.
- Все процессы используют общий `CREATE_NO_WINDOW` helper.

### 7.3 Project и pipeline

`ImportSource` различает `File(PathBuf)` и `Url(String)`. Remote source сначала скачивается во временный каталог, затем проходит тот же probe → decode → proxy → diarize → transcribe → merge.

- Local import сохраняет текущие phase weights.
- Remote import выделяет Download часть overall progress; остальные фазы масштабируются без регрессии монотонности.
- `SourceRef` получает backward-compatible optional `origin_url`; fingerprint вычисляется по скачанному файлу.
- Временный источник удаляется после успешного import; проект продолжает использовать собственный WAV/proxy. При ошибке partial проект и временные файлы очищаются.
- CLI получает тот же URL import, не отдельную несовместимую ветку логики.

### 7.4 Portable packaging

`fetch-win-deps.py` скачивает pinned official releases:

- standalone Windows x64 `yt-dlp.exe`;
- Windows x64 `deno.exe` версии ≥2.3.

Оба входят в REQUIRED, manifest, allowlist, ZIP и `DEPENDENCIES.txt`. NOTICE пополняется: yt-dlp — Unlicense, Deno — MIT; bundled EJS/runtime используются по их лицензиям. README явно требует скачивать только контент, который пользователь имеет право сохранять.

Полный ZIP остаётся offline после распаковки; сеть нужна только для самого remote media.

## 8. Ошибки и крайние случаи

- Пустой/corrupt proxy, unsupported codec, missing media: видимый error/empty state.
- `Range` за EOF: 416 с `Content-Range: bytes */size`; HEAD не читает body.
- Ссылка с редиректом: разрешена внутри yt-dlp; итоговый filepath берётся из его result.
- Playlist/multiple URLs: отказ с объяснением, без очереди.
- Повторный import во время активного job: заблокирован.
- Cancel на download/decode/transcribe: нет `.partial` проекта и orphan output.
- Длинное русское имя спикера и длинная реплика не ломают grid.
- Светлая и тёмная темы проверяются отдельно.

## 9. Verification

### Автоматические

- Rust unit/integration: URL validation, source mapping, downloader args/result/errors/cancel, range parser, localhost streaming, no whole-file body, project schema compatibility, phase ETA.
- Playwright: четыре grid rows, transcript/preview heights, video play/pause/seek/currentTime, timeline turn → active segment/scroll, long transcript, rename + undo, both themes, URL paste/drop, validation, processing labels.
- Downloader E2E с local HTTP direct media и fake yt-dlp для progress/cancel/errors; реальные online extractors не входят в flaky CI suite.
- Существующие `cargo test --workspace`, clippy `-D warnings`, fmt, Playwright, pytest, web release, Windows xwin release.
- Windows CI запускает packaging tests с новыми REQUIRED files.

### Ручная приёмка

- Тот же `7462162467573.mp4`: импорт, preview, playback, audio, seek, active transcript, speaker rename, timeline navigation.
- Direct HTTP media: drop и paste.
- По одному доступному публичному YouTube/Yandex/VK URL: download → transcript → playback. Если сервис требует login/anti-bot, зафиксировать точный результат, не выдавать за успех.
- Native Windows остаётся финальной приёмкой пользователя; Proton/WebView2 ограничения явно отделяются от web/mock результатов.

## 10. Definition of done

Все goals реализованы, автоматические проверки зелёные, native media не читает весь proxy в память, UI соответствует live screenshots, direct URL E2E завершён, portable ZIP содержит downloader/runtime. Результат передаётся пользователю на локальную проверку. Commit/push/tag/release — только после отдельного разрешения.
