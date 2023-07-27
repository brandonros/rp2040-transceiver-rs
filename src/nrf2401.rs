use crate::{bitfields::*, role::Role, led_state::LedState};

use embassy_sync::{channel::Sender, blocking_mutex::raw::ThreadModeRawMutex};
use embassy_time::{Duration, Timer};
use embedded_hal::{digital::OutputPin, spi::SpiDevice};

type Register = u8;
type Command = u8;

const R_REGISTER: Command = 0x00;
const W_REGISTER: Command = 0x20;
const R_RX_PAYLOAD: Command = 0x61;
const W_TX_PAYLOAD: Command = 0xA0;

const CONFIG: Register = 0x00;
const EN_AA: Register = 0x01;
const EN_RX_ADDR: Register = 0x02;
const SETUP_AW: Register = 0x03;
const SETUP_RETR: Register = 0x04;
const RF_CH: Register = 0x05;
const RF_SETUP: Register = 0x06;
const STATUS: Register = 0x07;
const RX_ADDR_P0: Register = 0x0A;
const TX_ADDR: Register = 0x10;
const RX_PW_P0: Register = 0x11;
const DYN_PD: Register = 0x1c;
const FEATURE: Register = 0x1d;
const FLUSH_TX: Register = 0xe1;
const FLUSH_RX: Register = 0xe2;

pub fn nrf24l01_write_register<SPI>(spi: &mut SPI, register: Register, byte: u8) -> (u8, u8)
where
    SPI: SpiDevice,
{
    defmt::info!("write_register({:02x}, {:02x})", register, byte);
    // For single byte registers only
    let request_buffer = [W_REGISTER | register, byte];
    let mut response_buffer = [0u8; 2];
    spi.transfer(&mut response_buffer, &request_buffer).unwrap();
    return (response_buffer[0], response_buffer[1]);
}

pub fn nrf24l01_read_register<SPI>(spi: &mut SPI, register: Register) -> (u8, u8)
where
    SPI: SpiDevice,
{
    // For single byte registers only
    let request_buffer = [R_REGISTER | register, 0];
    let mut response_buffer = [0u8; 2];
    spi.transfer(&mut response_buffer, &request_buffer).unwrap();
    return (response_buffer[0], response_buffer[1]);
}

pub async fn nrf2401_setup<Spi, CePin>(spi: &mut Spi, ce_output: &mut CePin, role: &Role)
where
    Spi: SpiDevice,
    CePin: OutputPin,
{
    // set CE low
    ce_output.set_low().unwrap();
    // sleep
    Timer::after(Duration::from_millis(5)).await;
    // SETUP_RETR set_retries
    let mut setup_retr = SetupRetr(0);
    setup_retr.set_auto_retransmit_delay(5);
    setup_retr.set_auto_retransmit_count(15);
    nrf24l01_write_register(spi, SETUP_RETR, setup_retr.0);
    // RF_SETUP set_data_rate or set_pa_level?
    let mut rf_setup = RfSteup(0);
    rf_setup.set_rf_pwr(3); // 0 = -18dBm, 1 = -12dBm, 2 = -6dBm, 3 = 0dBm
    rf_setup.set_rf_dr_low(0); // 0 = 1Mbps, 1 = 2Mbps, 3 = 250kbps, 4 = reserved
    rf_setup.set_rf_dr_high(0); // 0 = 1Mbps, 1 = 2Mbps, 3 = 250kbps, 4 = reserved
    nrf24l01_write_register(spi, RF_SETUP, rf_setup.0);
    // FEATURE disable_dynamic_payloads
    let mut feature = Feature(0);
    feature.set_ack_pay(0);
    feature.set_en_dpl(0);
    feature.set_en_dpl(0);
    nrf24l01_write_register(spi, FEATURE, feature.0);
    // DYNPD set_dynamic_payloads
    let mut dyn_pd = DynPd(0);
    dyn_pd.set_dpl_p0(0);
    nrf24l01_write_register(spi, DYN_PD, dyn_pd.0);
    // EN_AA disable_auto_ack
    let mut en_aa = EnAa(0);
    en_aa.set_enaa_p0(0);
    nrf24l01_write_register(spi, EN_AA, en_aa.0);
    // EN_RXADDR
    match role {
        Role::Transmitter => {
            // all RX pipes closed
            let mut en_rx_addr = EnRxAddr(0);
            en_rx_addr.set_erx_p0(0);
            nrf24l01_write_register(spi, EN_RX_ADDR, en_rx_addr.0);
        }
        Role::Receiver => {
            // only RX pipe0 opened
            let mut en_rx_addr = EnRxAddr(0);
            en_rx_addr.set_erx_p0(1);
            nrf24l01_write_register(spi, EN_RX_ADDR, en_rx_addr.0);
        }
    }
    // RX_PW_P0 set_payload_size
    let mut rx_pw_p0 = RxPwP0(0);
    rx_pw_p0.set_rx_pw_p0(32);
    nrf24l01_write_register(spi, RX_PW_P0, rx_pw_p0.0);
    // SETUP_AW set_address_width (weird 5 bytes - 2 = 3 thing)
    let mut setup_aw = SetupAw(0);
    setup_aw.set_aw(3); // 0 = illegal, 1 = 3 bytes, 2 = 4 bytes, 3 = 5 bytes
    nrf24l01_write_register(spi, SETUP_AW, setup_aw.0);
    // RF_CH // set_channel
    let mut rf_ch = RfCh(0);
    rf_ch.set_rf_ch(76);
    nrf24l01_write_register(spi, RF_CH, rf_ch.0);
    // NRF_STATUS // reset current status
    let mut status = Status(0);
    status.set_rx_dr(0);
    status.set_tx_ds(0);
    status.set_rx_p_no(0);
    status.set_max_rt(0); // TODO: usually 1?
    nrf24l01_write_register(spi, STATUS, status.0);
    // FLUSH_RX // flush
    nrf24l01_write_register(spi, FLUSH_RX, 0xff);
    // FLUSH_TX // flush
    nrf24l01_write_register(spi, FLUSH_TX, 0xff);
    // NRF_CONFIG
    match role {
        Role::Transmitter => {
            // clear config + power up + start as TX combined in one?
            let mut config = Config(0);
            config.set_prim_rx(0); // 0 = PTX, 1 = PRX
            config.set_crco(1);
            config.set_en_crc(1);
            config.set_pwr_up(1);
            nrf24l01_write_register(spi, CONFIG, config.0);
        }
        Role::Receiver => {
            let mut config = Config(0);
            config.set_prim_rx(1); // 0 = PTX, 1 = PRX
            config.set_crco(1);
            config.set_en_crc(1);
            config.set_pwr_up(1);
            nrf24l01_write_register(spi, CONFIG, config.0);
        }
    }
    // sleep
    Timer::after(Duration::from_micros(5000)).await;
    // TX_ADDR or RX_ADDR_P0
    match role {
        Role::Transmitter => {
            // write 5 byte TX_ADDR
            let mut response_buffer = [0u8; 6];
            let request_buffer = [
                W_REGISTER | TX_ADDR,
                1,
                0,
                0,
                0,
                0, // 5 byte pipe address
            ];
            spi.transfer(&mut response_buffer, &request_buffer).unwrap();
        }
        Role::Receiver => {
            // write 5 byte RX_ADDR_P0
            let mut response_buffer = [0u8; 6];
            let request_buffer = [
                W_REGISTER | RX_ADDR_P0,
                1,
                0,
                0,
                0,
                0, // 5 byte pipe address
            ];
            spi.transfer(&mut response_buffer, &request_buffer).unwrap();
        }
    }
    // CE pin
    match role {
        Role::Transmitter => {
            // do not bring it high?
        }
        Role::Receiver => {
            // bring it high
            ce_output.set_high().unwrap();
        }
    }
}

pub async fn nrf2401_receiver_loop<Spi>(spi: &mut Spi, led_sender: Sender<'static, ThreadModeRawMutex, LedState, 1>)
where
    Spi: SpiDevice,
{
    loop {
        // get status
        let (status, _) = nrf24l01_read_register(spi, STATUS); // NRF_STATUS
                                                             // parse
        let status = Status(status);
        let rx_dr = status.get_rx_dr();
        let is_data_available = rx_dr == 1;
        // read if data present
        if is_data_available {
            defmt::info!("status.rx_dr == 1");
            let mut response_buffer = [0u8; 33];
            let mut request_buffer: [u8; 33] = [0xFF; 33];
            request_buffer[0] = R_RX_PAYLOAD;
            spi.transfer(&mut response_buffer, &request_buffer).unwrap();
            defmt::info!("R_RX_PAYLOAD response_buffer = {:02x}", response_buffer[1..]);
            // write NRF_STATUS.RX_DR to clear
            let old_status = response_buffer[0];
            let mut new_status = Status(old_status);
            new_status.set_rx_dr(1);
            nrf24l01_write_register(spi, STATUS, new_status.0);
            // blink
            led_sender.send(LedState::On).await;
            Timer::after(Duration::from_millis(100)).await;
            led_sender.send(LedState::Off).await;
            Timer::after(Duration::from_millis(100)).await;
            // sleep
            Timer::after(Duration::from_millis(100)).await;
        }
    }
}

pub async fn nrf2401_transmitter_loop<Spi, CePin>(spi: &mut Spi, ce_output: &mut CePin, led_sender: Sender<'static, ThreadModeRawMutex, LedState, 1>)
where
    Spi: SpiDevice,
    CePin: OutputPin,
{
    loop {
        // transmit
        let message = b"Hello, world!";
        let mut request_buffer: [u8; 33] = [0; 33];
        request_buffer[0] = W_TX_PAYLOAD;
        for (i, byte) in message.iter().enumerate() {
            request_buffer[i + 1] = *byte;
        }
        let mut response_buffer = [0u8; 33];
        spi.transfer(&mut response_buffer, &request_buffer).unwrap();
        defmt::info!("W_TX_PAYLOAD response_buffer = {:02x}", response_buffer[1..]);
        // set CE high
        ce_output.set_high().unwrap();
        // sleep
        Timer::after(Duration::from_micros(10)).await;
        // set CE low
        ce_output.set_low().unwrap();
        // blink
        led_sender.send(LedState::On).await;
        Timer::after(Duration::from_millis(100)).await;
        led_sender.send(LedState::Off).await;
        Timer::after(Duration::from_millis(100)).await;
        // sleep
        Timer::after(Duration::from_millis(100)).await;
    }
}
