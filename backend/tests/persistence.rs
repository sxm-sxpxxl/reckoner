mod support;

use backend::db::meetings::{self, MeetingPatch, NewMeeting};
use backend::db::participants;
use backend::db::records::EntryKindRow;
use chrono::NaiveDate;
use support::test_pool;

/// Встреча-заготовка: почти каждому тесту нужна встреча и ничего больше.
async fn seed_meeting(tx: &mut sqlx::PgConnection) -> uuid::Uuid {
    meetings::insert(
        tx,
        NewMeeting {
            title: "Тестовая".to_owned(),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи")
    .id
}

#[tokio::test]
async fn migrations_apply_and_schema_is_queryable() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // Пять таблиц схемы должны существовать после миграций.
    let tables: Vec<String> = sqlx::query_scalar(
        "select table_name from information_schema.tables \
         where table_schema = 'public' and table_type = 'BASE TABLE' \
         order by table_name",
    )
    .fetch_all(&mut *tx)
    .await
    .expect("список таблиц");

    for expected in [
        "entries",
        "entry_shares",
        "meeting_log",
        "meetings",
        "participants",
    ] {
        assert!(
            tables.iter().any(|name| name == expected),
            "нет таблицы {expected}; есть: {tables:?}"
        );
    }
}

#[tokio::test]
async fn entry_kind_round_trips_through_a_text_column() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // Отображение перечисления на колонку `text` — то место, где компиляция
    // ничего не доказывает. Строки обязаны совпадать со значениями в CHECK
    // схемы, иначе вставка записи упадёт в рантайме, а не при сборке.
    for (kind, expected) in [
        (EntryKindRow::Expense, "expense"),
        (EntryKindRow::Transfer, "transfer"),
    ] {
        let encoded: String = sqlx::query_scalar("select $1::text")
            .bind(kind)
            .fetch_one(&mut *tx)
            .await
            .expect("кодирование в text");
        assert_eq!(encoded, expected, "kind {kind:?} закодировался не так");

        let decoded: EntryKindRow = sqlx::query_scalar("select $1::text")
            .bind(expected)
            .fetch_one(&mut *tx)
            .await
            .expect("декодирование из text");
        assert_eq!(decoded, kind, "строка {expected} раскодировалась не так");
    }
}

#[tokio::test]
async fn inserts_and_reads_back_a_meeting() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Дача у Влада".to_owned(),
            description: "Три дня, баня и продукты из «Ленты».".to_owned(),
            emoji: "🏡".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи");

    assert_eq!(created.title, "Дача у Влада");
    assert_eq!(created.emoji, "🏡");
    assert_eq!(created.cover_version, 0);
    assert!(created.cover_mime.is_none());

    let found = meetings::find(&mut tx, created.id)
        .await
        .expect("чтение встречи")
        .expect("встреча существует");

    assert_eq!(found.id, created.id);
    assert_eq!(found.held_on, created.held_on);
}

#[tokio::test]
async fn returns_none_for_a_missing_meeting() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let found = meetings::find(&mut tx, uuid::Uuid::nil())
        .await
        .expect("запрос выполнен");

    assert!(found.is_none());
}

#[tokio::test]
async fn patches_only_provided_fields() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Новая встреча".to_owned(),
            description: "черновик".to_owned(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка");

    let patched = meetings::update(
        &mut tx,
        created.id,
        MeetingPatch {
            title: Some("Солевые шашлыки".to_owned()),
            emoji: Some("🔥".to_owned()),
            ..MeetingPatch::default()
        },
    )
    .await
    .expect("правка")
    .expect("встреча существует");

    assert_eq!(patched.title, "Солевые шашлыки");
    assert_eq!(patched.emoji, "🔥");
    // Не переданные поля остаются как были.
    assert_eq!(patched.description, "черновик");
    assert_eq!(patched.held_on, created.held_on);
    // Правка двигает updated_at.
    assert!(patched.updated_at >= created.updated_at);
}

#[tokio::test]
async fn deletes_a_meeting_and_reports_whether_it_existed() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "На удаление".to_owned(),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка");

    assert!(
        meetings::delete(&mut tx, created.id)
            .await
            .expect("удаление")
    );
    assert!(
        !meetings::delete(&mut tx, created.id)
            .await
            .expect("повторное удаление")
    );
    assert!(
        meetings::find(&mut tx, created.id)
            .await
            .expect("чтение")
            .is_none()
    );
}

#[tokio::test]
async fn assigns_position_and_color_by_order_of_addition() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let first = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("первый участник");
    let second = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй участник");

    assert_eq!(first.position, 0);
    assert_eq!(second.position, 1);
    // Палитра аватаров на 8 цветов; индекс идёт по кругу.
    assert_eq!(first.color_index, 0);
    assert_eq!(second.color_index, 1);
}

#[tokio::test]
async fn lists_participants_in_position_order() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    for (name, emoji) in [("Настя", "🦊"), ("Влад", "🦉"), ("Егор", "🐸")] {
        participants::insert(&mut tx, meeting_id, name, emoji)
            .await
            .expect("участник");
    }

    let listed = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список");

    let names: Vec<&str> = listed.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Настя", "Влад", "Егор"]);
}

#[tokio::test]
async fn updates_and_deletes_a_participant() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let person = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("участник");

    let renamed = participants::update(&mut tx, person.id, "Анастасия", "🦩")
        .await
        .expect("правка")
        .expect("участник существует");
    assert_eq!(renamed.name, "Анастасия");
    assert_eq!(renamed.emoji, "🦩");
    // Позиция и цвет при правке не меняются.
    assert_eq!(renamed.position, person.position);
    assert_eq!(renamed.color_index, person.color_index);

    assert!(
        participants::delete(&mut tx, person.id)
            .await
            .expect("удаление")
    );
    assert!(
        participants::list_for_meeting(&mut tx, meeting_id)
            .await
            .expect("список")
            .is_empty()
    );
}

#[tokio::test]
async fn position_never_reuses_a_number_after_a_deletion() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let first = participants::insert(&mut tx, meeting_id, "Раз", "🐻")
        .await
        .expect("первый");
    participants::insert(&mut tx, meeting_id, "Два", "🦊")
        .await
        .expect("второй");
    participants::delete(&mut tx, first.id)
        .await
        .expect("удаление");

    // Номер берётся как max(position) + 1, поэтому третий получает 2, а не 1.
    // Счётчик по числу существующих участников выдал бы здесь 1 — уже занятый
    // «Два» номер — и вставка упала бы на уникальном индексе. Дырка на месте
    // удалённого остаётся навсегда, и это правильно.
    let third = participants::insert(&mut tx, meeting_id, "Три", "🐸")
        .await
        .expect("третий");
    assert_eq!(third.position, 2);

    let positions: Vec<i32> = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список")
        .iter()
        .map(|row| row.position)
        .collect();
    assert_eq!(positions, [1, 2]);
}
