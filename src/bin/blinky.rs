#![no_main]
#![no_std]

use rtic_usb as _; // global logger + panicking-behavior + memory layout

#[rtic::app(
    device = stm32h7xx_hal::pac, // TODO: Replace `some_hal::pac` with the path to the PAC
    dispatchers = [SPI1],  // TODO: Replace the `FreeInterrupt1, ...` with free interrupt vectors if software tasks are used
)]
mod app {
    use defmt::println;
    use dwt_systick_monotonic::{fugit::Duration, fugit::ExtU64, DwtSystick};
    use rtic::Monotonic;
    use stm32h7xx_hal::gpio::gpiob::{PB0, PB14};
    use stm32h7xx_hal::gpio::gpioc::PC13;
    use stm32h7xx_hal::gpio::gpioe::PE1;
    use stm32h7xx_hal::gpio::{Edge, ExtiPin, Input, Pull};
    use stm32h7xx_hal::gpio::{Output, PushPull};
    use stm32h7xx_hal::prelude::*;

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

        // IO
        let pb = cx
            .device
            .GPIOC
            .split(ccdr.peripheral.GPIOC)
            .pc13
            .into_pull_up_input();
        let gpiob = cx.device.GPIOB.split(ccdr.peripheral.GPIOB);
        let ld1 = gpiob.pb0.into_push_pull_output();
        let ld2 = cx
            .device
            .GPIOE
            .split(ccdr.peripheral.GPIOE)
            .pe1
            .into_push_pull_output();
        let ld3 = gpiob.pb14.into_push_pull_output();

        task1::spawn_after(ExtU64::millis(1000));

        // Setup the monotonic timer
        (
            Shared {
                ld2,
                // Initialization of shared resources go here
            },
            Local {
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
        defmt::info!("idle");
        loop {
            cortex_m::asm::nop();
        }
    }

    // TODO: Add tasks
    #[task(shared = [ld2])]
    fn task1(mut cx: task1::Context) {
        task2::spawn_after(ExtU64::millis(1000));
        cx.shared.ld2.lock(|ld2| {
            ld2.set_high();
        });
        //let time = monotonics::now();
        defmt::info!("task1! @ {}", monotonics::now().ticks());
    }

    #[task(shared = [ld2])]
    fn task2(mut cx: task2::Context) {
        task1::spawn_after(ExtU64::millis(1000));
        cx.shared.ld2.lock(|ld2| {
            ld2.set_low();
        });
        defmt::info!("task2! @ {}", monotonics::now().ticks());
    }
}
