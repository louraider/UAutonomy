//! Заготовка headless-хаба. Згодом тут буде локальний HTTP-сервер, який
//! роздає бібліотеку телефонам у Wi-Fi-мережі (вхід через QR-код).

fn main() {
    println!("uautonomy-hub {} (каркас)", uautonomy_core::version());
}
