extern crate imap;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::ConnectOptions;
use std::str::FromStr;

pub struct ConnectionDetails {
    account_id: i64,
    name: String,
    email: String,
    password: String,
    imap_server: String,
    imap_port: i64,
    smtp_server: String,
    smtp_port: i64,
}

async fn get_conn(database: &str) -> sqlx::SqliteConnection {
    SqliteConnectOptions::from_str(database)
        .unwrap()
        .journal_mode(SqliteJournalMode::Wal)
        .read_only(true)
        .connect()
        .await
        .unwrap()
}

pub async fn connect(database: &str, account_id: i64) -> ConnectionDetails {
    let mut conn = get_conn(database).await;
    let account = sqlx::query!("SELECT * FROM accounts WHERE id = $1", account_id)
        .fetch_one(&mut conn)
        .await
        .unwrap();

    ConnectionDetails {
        account_id: account.id,
        name: account.name,
        email: account.email,
        password: account.password,
        imap_server: account.imap_host,
        imap_port: account.imap_port,
        smtp_server: account.smtp_host,
        smtp_port: account.smtp_port,
    }
}

pub async fn get_emails(
    database: &str,
    details: ConnectionDetails,
) -> imap::error::Result<Option<String>> {
    let domain = details.imap_server;
    let client = imap::ClientBuilder::new(domain, details.imap_port.try_into().unwrap())
        .starttls()
        .native_tls()
        .expect("Could not connect to server");

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login(details.email, details.password)
        .map_err(|e| e.0)?;

    // we want to fetch the first email in the INBOX mailbox
    imap_session.select("INBOX")?;

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let messages = imap_session.fetch("1", "RFC822")?;
    let message = if let Some(m) = messages.iter().next() {
        m
    } else {
        return Ok(None);
    };

    // extract the message's body
    let body = message.body().expect("message did not have a body!");
    let body = std::str::from_utf8(body)
        .expect("message was not valid utf-8")
        .to_string();

    // be nice to the server and log out
    imap_session.logout()?;

    let mut conn = get_conn(database).await;

    sqlx::query!(
        "INSERT INTO emails (account, body) VALUES ($1, $2)",
        details.account_id,
        body
    )
    .execute(&mut conn)
    .await
    .unwrap();

    Ok(Some(body))
}
