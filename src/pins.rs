// UART0 (serial logs)
macro_rules! get_uart0_tx_pin { ($p:expr) => { $p.PIN_16 }; } // 21, UART0 TX (blue)
macro_rules! get_uart0_rx_pin { ($p:expr) => { $p.PIN_17 }; } // 22, UART0 RX (white)
pub(crate) use get_uart0_tx_pin;
pub(crate) use get_uart0_rx_pin;

// SPI0 (receiver)
macro_rules! get_spi0_sck_pin { ($p:expr) => { $p.PIN_18 }; } // 24, SPI0 SCK (yellow)
macro_rules! get_spi0_mosi_pin { ($p:expr) => { $p.PIN_19 }; } // 25, SPI0 MOSI TX (orange)
macro_rules! get_spi0_miso_pin { ($p:expr) => { $p.PIN_20 }; } // 26, SPI0 MISO RX (green)
macro_rules! get_spi0_cs_pin { ($p:expr) => { $p.PIN_21 }; } // 27, SPI0 CSn (purple)
macro_rules! get_receiver_ce_pin { ($p:expr) => { $p.PIN_22 }; } // 29 (brown)
pub(crate) use get_spi0_sck_pin;
pub(crate) use get_spi0_mosi_pin;
pub(crate) use get_spi0_miso_pin;
pub(crate) use get_spi0_cs_pin;
pub(crate) use get_receiver_ce_pin;

// SPI1 (transmitter)
macro_rules! get_transmitter_ce_pin { ($p:expr) => { $p.PIN_9 }; } // 12 (brown)
macro_rules! get_spi1_sck_pin { ($p:expr) => { $p.PIN_10 }; } // 14, SPI1 SCK (yellow)
macro_rules! get_spi1_mosi_pin { ($p:expr) => { $p.PIN_11 }; } // 15, SPI1 MISO TX (orange)
macro_rules! get_spi1_miso_pin { ($p:expr) => { $p.PIN_12 }; } // 16, SPI1 MOSI RX (green)
macro_rules! get_spi1_cs_pin { ($p:expr) => { $p.PIN_13 }; } // 17, SPI1 CSn (purple)
pub(crate) use get_transmitter_ce_pin;
pub(crate) use get_spi1_sck_pin;
pub(crate) use get_spi1_mosi_pin;
pub(crate) use get_spi1_miso_pin;
pub(crate) use get_spi1_cs_pin;

// LED
macro_rules! get_led_pin { ($p:expr) => { $p.PIN_25 }; } // built in
pub(crate) use get_led_pin;
