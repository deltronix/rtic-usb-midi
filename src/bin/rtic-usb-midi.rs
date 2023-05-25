#![no_main]
#![no_std]

use rtic_usb_midi as _; // global logger + panicking-behavior + memory layout

#[rtic::app(
    device = stm32h7xx_hal::pac, // TODO: Replace `some_hal::pac` with the path to the PAC
    dispatchers = [SPI1],  // TODO: Replace the `FreeInterrupt1, ...` with free interrupt vectors if software tasks are used
)]
mod app {
    use defmt::println;
    use dwt_systick_monotonic::{fugit::Duration, fugit::ExtU64, DwtSystick};
    use midly_usb::live::{SystemCommon, SystemRealtime};
    use midly_usb::{live::LiveEvent, MidiDevice, MidiMessage, UsbMidiPacket};
    use rtic::Monotonic;
    use stm32h7xx_hal::gpio::gpiob::{PB0, PB14};
    use stm32h7xx_hal::gpio::gpioc::PC13;
    use stm32h7xx_hal::gpio::gpioe::PE1;
    use stm32h7xx_hal::gpio::{Edge, ExtiPin, Input, Pull};
    use stm32h7xx_hal::gpio::{Output, PushPull};
    use stm32h7xx_hal::prelude::*;
    use stm32h7xx_hal::rcc::rec::UsbClkSel;
    use stm32h7xx_hal::usb_hs::{UsbBus, USB2};
    use usb_device::prelude::*;

    static mut EP_MEMORY: [u32; 1024] = [0; 1024];

    // TODO: Add a monotonic if scheduling will be used
    #[monotonic(binds = SysTick, default = true)]
    type Mono = DwtSystick<80_000_000>;

    use super::*;

    // Shared resources go here
    #[shared]
    struct Shared {
        ld2: PE1<Output<PushPull>>,
        // TODO: Add resources
    }

    // Local resources go here
    #[local]
    struct Local {
        usb: (
            UsbDevice<'static, UsbBus<USB2>>,
            MidiDevice<'static, UsbBus<USB2>>,
        ),
        // TODO: Add resources
        pb: PC13<Input>,
        ld1: PB0<Output<PushPull>>,
        ld3: PB14<Output<PushPull>>,
    }

    #[init]
    fn init(mut cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");

        let pwr = cx.device.PWR.constrain();
        let pwrcfg = pwr.freeze();

        let rcc = cx.device.RCC.constrain();
        let mut ccdr = rcc.sys_ck(80.MHz()).freeze(pwrcfg, &cx.device.SYSCFG);

        let dcb = &mut cx.core.DCB;
        let dwt = cx.core.DWT;

        let systick = cx.core.SYST;
        let mono = Mono::new(dcb, dwt, systick, 80_000_000);

        let _ = ccdr.clocks.hsi48_ck().expect("HSI48 required");
        ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::Hsi48);

        // IO
        let (usb_dm, usb_dp) = {
            let gpioa = cx.device.GPIOA.split(ccdr.peripheral.GPIOA);
            (gpioa.pa11.into_alternate(), gpioa.pa12.into_alternate())
        };
        let pb = cx
            .device
            .GPIOC
            .split(ccdr.peripheral.GPIOC)
            .pc13
            .into_pull_up_input();
        let mut gpiob = cx.device.GPIOB.split(ccdr.peripheral.GPIOB);
        let ld1 = gpiob.pb0.into_push_pull_output();
        let ld2 = cx
            .device
            .GPIOE
            .split(ccdr.peripheral.GPIOE)
            .pe1
            .into_push_pull_output();
        let ld3 = gpiob.pb14.into_push_pull_output();

        let usb = USB2::new(
            cx.device.OTG2_HS_GLOBAL,
            cx.device.OTG2_HS_DEVICE,
            cx.device.OTG2_HS_PWRCLK,
            usb_dm,
            usb_dp,
            ccdr.peripheral.USB2OTG,
            &ccdr.clocks,
        );

        let usb_bus =
            cortex_m::singleton!( : usb_device::class_prelude::UsbBusAllocator<UsbBus<USB2>> =
                                            UsbBus::new(usb, unsafe { &mut EP_MEMORY} ) )
            .unwrap();

        let usb_midi = MidiDevice::new(usb_bus);
        let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("deltronix")
            .product("tester")
            .serial_number("test")
            .device_class(midly_usb::class::USB_CLASS_AUDIO)
            .build();
        let usb = (usb_dev, usb_midi);
        task1::spawn_after(ExtU64::millis(1000));

        // Setup the monotonic timer
        (
            Shared {
                ld2,
                // Initialization of shared resources go here
            },
            Local {
                usb,
                pb,
                ld1,
                ld3,
                // Initialization of local resources go here
            },
            init::Monotonics(
                mono, // Initialization of optional monotonic timers go here
            ),
        )
    }
    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }

    // TODO: Add tasks
    #[task(shared = [ld2])]
    fn task1(mut cx: task1::Context) {
        cx.shared.ld2.lock(|ld2| {
            ld2.set_high();
        });
        let time = monotonics::now();
        defmt::println!("task1! @ {}", time.ticks());
        task2::spawn_after(ExtU64::millis(1000));
    }

    #[task(shared = [ld2])]
    fn task2(mut cx: task2::Context) {
        cx.shared.ld2.lock(|ld2| {
            ld2.set_low();
        });
        defmt::println!("task2!");
        task1::spawn_after(ExtU64::millis(1000));
    }

    #[task(binds = OTG_FS, local = [usb,ld1])]
    fn usb_event(mut cx: usb_event::Context) {
        let (usb_dev, usb_midi) = &mut cx.local.usb;
        cx.local.ld1.set_high();

        loop {
            if !usb_dev.poll(&mut [usb_midi]) {
                cx.local.ld1.set_low();
                return;
            }

            let mut buf = [0u8; 64];

            match usb_midi.read(&mut buf) {
                Ok(count) if count > 0 => {
                    defmt::println!("{}", count);
                    let mut write_offset = 0;
                    let slice = &buf[0..count];
                    slice.chunks(4).for_each(|packet| {
                        defmt::println!(
                            "{:X} {:X} {:X} {:X}",
                            packet[0],
                            packet[1],
                            packet[2],
                            packet[3]
                        );
                        let now = monotonics::now().duration_since_epoch().to_millis();

                        let ube = UsbMidiPacket::read(packet);

                        defmt::println!(
                            "{:X} {:?}",
                            ube.cable_number.as_int(),
                            ube.code_index_number.as_int()
                        );
                        match ube.event {
                            LiveEvent::Midi { channel, message } => match message {
                                MidiMessage::NoteOn { key, vel } => {
                                    defmt::println!("k: {}, v: {}", key.as_int(), vel.as_int());
                                }
                                MidiMessage::NoteOff { key, vel } => {
                                    println!("k: {}, v: {}", key.as_int(), vel.as_int());
                                }
                                _ => {
                                    defmt::println!("unimplemented midi message")
                                }
                            },
                            LiveEvent::Realtime(status) => match status {
                                SystemRealtime::TimingClock => {
                                    defmt::println!("clk");
                                }
                                _ => {
                                    defmt::println!("sys rt");
                                }
                            },
                            _ => {
                                defmt::println!("unimplemented midi event")
                            }
                        }
                    });
                    while write_offset < count {
                        match usb_midi.write(&buf[write_offset..count]) {
                            Ok(len) if len > 0 => {
                                write_offset += len;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
