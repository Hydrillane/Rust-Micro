// @generated automatically by Diesel CLI.

diesel::table! {
    messages (id) {
        id -> Int4,
        username -> Varchar,
        message -> Text,
        timestamp -> Int8,
    }
}
