#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod bitfields;
mod nrf2401;
mod role;
mod pins;
mod led_state;

use core::cell::RefCell;

use defmt_serial as _;
use embassy_executor::Executor;
use embassy_rp::{
    bind_interrupts,
    gpio::{self, Level, AnyPin, Pin},
    multicore::{spawn_core1, Stack},
    peripherals::{SPI0, SPI1, UART0},
    spi::{self, Phase, Polarity, Spi},
    uart::{self, InterruptHandler},
};
use embassy_sync::{blocking_mutex::{raw::{NoopRawMutex, ThreadModeRawMutex}, Mutex}, channel::{Receiver, Channel, Sender}};

use panic_probe as _;
use role::Role;
use static_cell::StaticCell;

use crate::led_state::LedState;

bind_interrupts!(struct Irqs {
    UART0_IRQ => InterruptHandler<UART0>;
});

static mut CORE1_STACK: Stack<4096> = Stack::new();
static CORE0_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static LED_CHANNEL: StaticCell<Channel<ThreadModeRawMutex, LedState, 1>> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    // UART0
    let uart0 = uart::Uart::new(
        p.UART0,
        pins::get_uart0_tx_pin!(p),
        pins::get_uart0_rx_pin!(p),
        Irqs,      // unused?
        p.DMA_CH0, // unused?
        p.DMA_CH1, // unused?
        uart::Config::default(),
    );
    // defmt serial
    defmt_serial::defmt_serial(uart0);
    // LED channel
    let led_channel = LED_CHANNEL.init(Channel::new());
    let led_channel_receiver = led_channel.receiver();
    let led_channel_sender1 = led_channel.sender();
    let led_channel_sender2 = led_channel.sender();
    // SPI0 SPI_MODE0 config
    let mut spi0_config = spi::Config::default();
    spi0_config.frequency = 10_000_000;
    spi0_config.polarity = Polarity::IdleLow;
    spi0_config.phase = Phase::CaptureOnFirstTransition;
    // SPI0 bus
    let spi0_bus = Spi::new_blocking(
        p.SPI0,
        pins::get_spi0_sck_pin!(p), 
        pins::get_spi0_mosi_pin!(p),
        pins::get_spi0_miso_pin!(p),
        spi0_config,
    );
    // SPI1 SPI_MODE0 config
    let mut spi1_config = spi::Config::default();
    spi1_config.frequency = 10_000_000;
    spi1_config.polarity = Polarity::IdleLow;
    spi1_config.phase = Phase::CaptureOnFirstTransition;
    // spi1 bus
    let spi1_bus = Spi::new_blocking(
        p.SPI1,
        pins::get_spi1_sck_pin!(p), 
        pins::get_spi1_mosi_pin!(p),
        pins::get_spi1_miso_pin!(p),
        spi1_config,
    );
    // spawn tasks (core1)
    spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let core1_executor = CORE1_EXECUTOR.init(Executor::new());
        core1_executor.run(|spawner| {
            spawner.must_spawn(nrf2401_receiver_task(
                spi0_bus, 
                pins::get_spi0_cs_pin!(p).degrade(), 
                pins::get_receiver_ce_pin!(p).degrade(),
                led_channel_sender1
            ));
        });
    });
    // spawn tasks (core0)
    let core0_executor = CORE0_EXECUTOR.init(Executor::new());
    core0_executor.run(|spawner| {
        spawner.must_spawn(led_task(
            pins::get_led_pin!(p).degrade(),
            led_channel_receiver
        ));
        spawner.must_spawn(nrf2401_transmitter_task(
            spi1_bus, 
            pins::get_spi1_cs_pin!(p).degrade(), 
            pins::get_transmitter_ce_pin!(p).degrade(),
            led_channel_sender2
        ));
    });
}

#[embassy_executor::task]
async fn led_task(
    led_pin: AnyPin,
    receiver: Receiver<'static, ThreadModeRawMutex, LedState, 1>
) {
    let mut led_output = gpio::Output::new(led_pin, Level::Low);
    loop {
        match receiver.recv().await {
            LedState::On => led_output.set_high(),
            LedState::Off => led_output.set_low(),
        }
    }
}

#[embassy_executor::task]
async fn nrf2401_receiver_task(
    spi0_bus: embassy_rp::spi::Spi<'static, SPI0, embassy_rp::spi::Blocking>,
    spi0_bus_cs_pin: AnyPin,
    ce_pin: AnyPin,
    led_sender: Sender<'static, ThreadModeRawMutex, LedState, 1>
) {
    // SPI0
    let mut spi0_bus_cs_output = gpio::Output::new(spi0_bus_cs_pin, Level::High); 
    spi0_bus_cs_output.set_high();
    let spi0_bus = Mutex::<NoopRawMutex, _>::new(RefCell::new(spi0_bus));
    let mut spi0_device = embassy_embedded_hal::shared_bus::blocking::spi::SpiDevice::new(&spi0_bus, spi0_bus_cs_output);
    // NRF2401
    let mut ce_output = gpio::Output::new(ce_pin, Level::Low);
    ce_output.set_low();
    let role = Role::Receiver;
    nrf2401::nrf2401_setup(&mut spi0_device, &mut ce_output, &role).await;
    nrf2401::nrf2401_receiver_loop(&mut spi0_device, led_sender).await;
}

#[embassy_executor::task]
async fn nrf2401_transmitter_task(
    spi1_bus: embassy_rp::spi::Spi<'static, SPI1, embassy_rp::spi::Blocking>,
    spi1_bus_cs_pin: AnyPin,
    ce_pin: AnyPin,
    led_sender: Sender<'static, ThreadModeRawMutex, LedState, 1>    
) {
    // SPI1
    let mut spi1_bus_cs_output = gpio::Output::new(spi1_bus_cs_pin, Level::High); 
    spi1_bus_cs_output.set_high();
    let spi1_bus = Mutex::<NoopRawMutex, _>::new(RefCell::new(spi1_bus));
    let mut spi1_device = embassy_embedded_hal::shared_bus::blocking::spi::SpiDevice::new(&spi1_bus, spi1_bus_cs_output);
    // NRF2401
    let mut ce_output = gpio::Output::new(ce_pin, Level::Low);
    ce_output.set_low();
    let role = Role::Transmitter;
    nrf2401::nrf2401_setup(&mut spi1_device, &mut ce_output, &role).await;
    nrf2401::nrf2401_transmitter_loop(&mut spi1_device, &mut ce_output, led_sender).await;
}
