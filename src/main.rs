use reqwest::header::{USER_AGENT, ACCEPT, ACCEPT_LANGUAGE};
use scraper::{Html, Selector};
use rusqlite::{params, Connection};
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;

// Type alias pour simplifier la gestion d erreur multithreadée
type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;

#[derive(Debug, Clone)]
struct Product {
    rank: i32,
    title: String,
    price: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = "db";
    setup_db(db_path)?;

    println!("🕵️  Sentinelle activée. Surveillance des opportunités toutes les 2h...");

    loop {
        let now = Utc::now().format("%H:%M:%S").to_string();
        println!("\n--- 🕒 CYCLE DE SCAN : {} ---", now);
        
        if let Err(e) = run_scan(db_path).await {
            eprintln!("❌ Erreur lors du scan : {}", e);
        }
        
        println!("\n😴 Sommeil pour 2 heures...");
        sleep(Duration::from_secs(7200)).await;
    }
}

async fn run_scan(db_path: &str) -> Result<()> {
    let categories = vec![
        ("Electronics", "https://www.amazon.fr/gp/movers-and-shakers/electronics"),
        ("Kitchen", "https://www.amazon.fr/gp/movers-and-shakers/kitchen"),
        ("Home", "https://www.amazon.fr/gp/movers-and-shakers/home"),
        ("Video Games", "https://www.amazon.fr/gp/movers-and-shakers/videogames"),
        ("Tools", "https://www.amazon.fr/gp/movers-and-shakers/hi"),
    ];

    for (name, url) in categories {
        let products = scrape_category(url).await?;
        let mut conn = Connection::open(db_path).map_err(|e| Box::new(e) as GenericError)?;
        let tx = conn.transaction().map_err(|e| Box::new(e) as GenericError)?;

        println!("\n📈 Analyse {}", name);
        
        for p in products {
            let prev: Option<(i32, String)> = tx.query_row(
                "SELECT rank, price FROM rankings WHERE title = ?1 AND category = ?2 ORDER BY captured_at DESC LIMIT 1",
                params![p.title, name],
                |row| Ok((row.get(0)?, row.get::<_, Option<String>>(1)?.unwrap_or_default())),
            ).ok();

            if let Some((old_rank, _)) = prev {
                let gain = old_rank - p.rank;
                if gain >= 10 {
                    println!("🚀 ALERTE WINNER : [+{} places] - {} ({}€)", gain, p.title, p.price.as_deref().unwrap_or("?"));
                }
            } else if p.rank <= 20 {
                println!("✨ NOUVEAU TOP 20 : {} ({}€)", p.title, p.price.as_deref().unwrap_or("?"));
            }

            tx.execute(
                "INSERT INTO rankings (category, rank, title, price, captured_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, p.rank, p.title, p.price, Utc::now().to_rfc3339()],
            )?;
        }
        tx.commit().map_err(|e| Box::new(e) as GenericError)?;
        sleep(Duration::from_millis(1500)).await;
    }
    Ok(())
}

fn setup_db(path: &str) -> std::result::Result<(), rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.execute("CREATE TABLE IF NOT EXISTS rankings (id INTEGER PRIMARY KEY, category TEXT, rank INTEGER, title TEXT, price TEXT, captured_at DATETIME)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_fast ON rankings (title, category)", [])?;
    Ok(())
}

async fn scrape_category(url: &str) -> Result<Vec<Product>> {
    let client = reqwest::Client::new();
    let response = client.get(url)
        .header(USER_AGENT, "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "fr-FR,fr;q=0.9")
        .send().await?.text().await?;

    let document = Html::parse_document(&response);
    let card_selector = Selector::parse("div[role=\"treeitem\"]").unwrap();
    let img_selector = Selector::parse("img").unwrap(); 
    let price_selector = Selector::parse(".p13n-sc-price, ._cDE3f_price_1y972").unwrap();

    let mut products = Vec::new();
    let mut rank = 1;

    for card in document.select(&card_selector) {
        let title = card.select(&img_selector).next()
            .and_then(|img| img.value().attr("alt"))
            .filter(|alt| alt.len() > 10 && !alt.contains("arrow"))
            .map(|alt| alt.trim().to_string());

        if let Some(t) = title {
            let price = card.select(&price_selector).next().map(|p| p.text().collect::<String>());
            products.push(Product { rank, title: t, price });
            rank += 1;
        }
    }
    Ok(products)
}
