//! Software PID controller
//!
//! Currently in a slightly experimental state, if you are attaching this to
//! anything can break, please sit right next to the emergency stop (and read
//! the source).
//!
//! Any change in behaviour (even bugfixes) in this will result in a major
//! version increase (0.a -> 0.a+1 or x.y.z -> x+1.0.0), so upgrading with `^`
//! will not break your carefully set tunings.
//!
//! Owes a great debt to:
//!
//! * https://en.wikipedia.org/wiki/PID_controller
//! * http://www.embedded.com/design/prototyping-and-development/4211211/PID-without-a-PhD
//! * http://brettbeauregard.com/blog/2011/04/improving-the-beginners-pid-introduction/

// FIXME: it may be worth to explore http://de.mathworks.com/help/simulink/slref/pidcontroller.html
//        for additional features/inspiration

extern crate core;

mod util;

use std::f64;

/// A generic controller interface.
///
/// A controller is fed timestamped values and calculates an adjusted value
/// based on previous readings.
///
/// Many controllers possess a set of adjustable parameters, as well as a set
/// of input-value dependant state variables.
pub trait Controller {
    /// Record a measurement from the plan.
    ///
    /// Records a new values. `delta_t` is the time since the last update in
    /// seconds.
    fn update(&mut self, value: f64, delta_t: f64) -> f64;

    /// Adjust set target for the plant.
    ///
    /// The controller will usually try to adjust its output (from `update`) in
    /// a way that results in the plant approaching `target`.
    fn set_target(&mut self, target: f64);

    /// Retrieve target value.
    fn target(&self) -> f64;

    /// Reset internal state.
    ///
    /// Resets the internal state of the controller; not to be confused with
    /// its parameters.
    fn reset(&mut self);
}


/// PID controller derivative modes.
///
/// Two different ways of calculating the derivative can be used with the PID
/// controller, allowing to avoid "derivative kick" if needed (see
/// http://brettbeauregard.com/blog/2011/04/improving-the-beginner%E2%80%99s-pid-derivative-kick/
/// for details information on the implementation that inspired this one).
///
/// Choosing `OnMeasurement` will avoid large bumps in the controller output
/// when changing the setpoint using `set_target()`.
#[derive(Debug, Clone, Copy)]
pub enum DerivativeMode {
    /// Calculate derivative of error (classic PID-Controller)
    OnError,
    /// Calculate derivative of actual changes in value.
    OnMeasurement,
}

/// PID Controller.
///
/// A PID controller, supporting the `Controller` interface. Any public values
/// are safe to modify while in operation.
///
/// `p_gain`, `i_gain` and `d_gain` are the respective gain values. The
/// controlller internally stores an already adjusted integral, making it safe
/// to alter the `i_gain` - it will *not* result in an immediate large jump in
/// controller behavior.
///
/// `i_min` and `i_max` are the limits for the internal integral storage.
/// Similarly, `out_min` and `out_max` clip the output value to an acceptable
/// range of values. By default, all limits are set to +/- infinity.
///
/// `d_mode` The `DerivativeMode`, the default is `OnMeasurement`.
#[derive(Debug, Clone)]
pub struct PIDController {
    /// Proportional gain
    pub p_gain: f64,

    /// Integral gain
    pub i_gain: f64,

    /// Differential gain,
    pub d_gain: f64,

    target: f64,

    // Integral range limits
    pub i_min: f64,
    pub i_max: f64,

    // Output range limits
    pub out_min: f64,
    pub out_max: f64,

    pub d_mode: DerivativeMode,

    // The PIDs internal state. All other attributes are configuration values
    err_sum: f64,
    prev_value: f64,
    prev_error: f64,
}

impl PIDController {
    /// Creates a new PID Controller.
    pub fn new(p_gain: f64, i_gain: f64, d_gain: f64) -> PIDController {
        PIDController{
            p_gain: p_gain,
            i_gain: i_gain,
            d_gain: d_gain,

            target: 0.0,

            err_sum: 0.0,
            prev_value: f64::NAN,
            prev_error: f64::NAN,

            i_min: -f64::INFINITY,
            i_max: f64::INFINITY,

            out_min: -f64::INFINITY,
            out_max: f64::INFINITY,

            d_mode: DerivativeMode::OnMeasurement,
        }
    }

    /// Convenience function to set `i_min`/`i_max` and `out_min`/`out_max`
    /// to the same values simultaneously.
    pub fn limits(&mut self, min: f64, max: f64) {
        self.i_min = min;
        self.i_max = max;
        self.out_min = min;
        self.out_max = max;
    }
}

impl Controller for PIDController {
    fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    fn target(&self) -> f64 {
        self.target
    }

    fn update(&mut self, value: f64, delta_t: f64) -> f64 {
        let error = self.target - value;

        // PROPORTIONAL
        let p_term = self.p_gain * error;

        // INTEGRAL
        self.err_sum += util::limit_range(
            self.i_min, self.i_max,
            self.err_sum + self.i_gain * error * delta_t
        );
        let i_term = self.err_sum;

        // DIFFERENTIAL
        let d_term = if self.prev_value == f64::NAN ||
                        self.prev_error == f64::NAN {
            // we have no previous values, so skip the derivative calculation
            0.0
        } else {
            match self.d_mode {
                DerivativeMode::OnMeasurement => {
                    // we use -delta_v instead of delta_error to reduce "derivative kick",
                    // see http://brettbeauregard.com/blog/2011/04/improving-the-beginner%E2%80%99s-pid-derivative-kick/
                    self.d_gain * (self.prev_value - value) / delta_t
                },
                DerivativeMode::OnError => {
                    self.d_gain * (error - self.prev_error) / delta_t
                }
            }
        };

        // store previous values
        self.prev_value = value;
        self.prev_error = error;

        util::limit_range(
            self.out_min, self.out_max,
            p_term + d_term + i_term
        )
    }

    fn reset(&mut self) {
        self.prev_value = f64::NAN;
        self.prev_error = f64::NAN;

        // FIXME: http://brettbeauregard.com/blog/2011/04/improving-the-beginner
        //               %E2%80%99s-pid-initialization/
        //        suggests that this should not be there. however, it may miss
        //        the fact that input and output can be two completely
        //        different domains
        self.err_sum = 0.0;
    }
}
