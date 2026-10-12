use rail_httpx_core::db::Database;

#[tokio::test]
async fn sqlite_crud_via_any_driver() {
    let db = Database::connect_in_memory_sqlite().await.unwrap();

    db.query("CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT)")
        .await
        .unwrap();
    db.query("INSERT INTO items (label) VALUES ('a'),('b'),('c')")
        .await
        .unwrap();

    let out = db
        .query("SELECT id, label FROM items WHERE label != 'b' ORDER BY id")
        .await
        .unwrap();

    assert_eq!(out.rows.len(), 2);
    assert_eq!(out.rows[0]["label"], "a");
    assert_eq!(out.rows[1]["label"], "c");
}