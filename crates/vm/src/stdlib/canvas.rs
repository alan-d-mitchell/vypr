use error::error::VyprError;

use crate::value::{Module, NativeFunction, Value};
use crate::stdlib::error;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::rc::Rc;

thread_local! {
    static CANVAS_WIDTH: RefCell<usize> = const { RefCell::new(0) };
    static CANVAS_HEIGHT: RefCell<usize> = const { RefCell::new(0) };
    static CANVAS_PIXELS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub fn create_module() -> Value {
    let mut exports = HashMap::new();

    exports.insert("init".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "init".to_string(),
        function: canvas_init,
    })));

    exports.insert("set_pixel".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "set_pixel".to_string(),
        function: canvas_set_pixel,
    })));

    exports.insert("save".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "save".to_string(),
        function: canvas_save,
    })));

    exports.insert("draw_rect".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "draw_rect".to_string(),
        function: canvas_draw_rect,
    })));

    exports.insert("draw_circle".to_string(), Value::Native(Rc::new(NativeFunction {
        name: "draw_circle".to_string(),
        function: canvas_draw_circle,
    })));

    Value::Module(Rc::new(Module {
        name: "canvas".to_string(),
        exports,
    }))
}

fn canvas_init(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 2 {
        return Err(error("R014", format!("init() takes exactly 2 arguments, got {}", args.len())));
    }

    if let (Value::Int(w), Value::Int(h)) = (&args[0], &args[1]) {
        let width = *w as usize;
        let height = *h as usize;

        CANVAS_WIDTH.with(|cw| *cw.borrow_mut() = width);
        CANVAS_HEIGHT.with(|ch| *ch.borrow_mut() = height);
        
        CANVAS_PIXELS.with(|cp| {
            *cp.borrow_mut() = vec![0; width * height * 3];
        });
        Ok(Value::None)
    } else {
        Err(error("R002", "arguments to 'init()' must be of type 'int'"))
    }
}

fn canvas_set_pixel(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 5 {
        return Err(error("R014", format!("set_pixel() takes exactly 5 arguments, got {}", args.len())));
    }

    if let (Value::Int(x), Value::Int(y), 
        Value::Int(r), Value::Int(g), Value::Int(b)
    ) = (&args[0], &args[1], &args[2], &args[3], &args[4]) 
    {
        let x = *x as usize;
        let y = *y as usize;

        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                let width = *cw.borrow();
                let height = *ch.borrow();

                if x < width && y < height {
                    let idx = (y * width + x) * 3;

                    CANVAS_PIXELS.with(|cp| {
                        let mut pixels = cp.borrow_mut();
                        pixels[idx] = *r as u8;
                        pixels[idx + 1] = *g as u8;
                        pixels[idx + 2] = *b as u8;
                    });
                }
            })
        });

        Ok(Value::None)
    } else {
        Err(error("R002", "arguments to 'set_pixel()' must be of type 'int'"))
    }
}

fn canvas_save(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 1 {
        return Err(error("R014", format!("save() takes exactly 1 argument, got {}", args.len())));
    }

    if let Value::Str(filename) = &args[0] {
        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                CANVAS_PIXELS.with(|cp| {
                    let width = *cw.borrow();
                    let height = *ch.borrow();
                    let pixels = cp.borrow();

                    if let Ok(file) = File::create(filename.as_str()) {
                        let mut writer = BufWriter::new(file);
                        let _ = writeln!(writer, "P3\n{} {}\n255", width, height);
                        
                        for chunk in pixels.chunks(3) {
                            let _ = writeln!(writer, "{} {} {}", chunk[0], chunk[1], chunk[2]);
                        }
                    }
                })
            })
        });

        Ok(Value::None)
    } else {
        Err(error("R002", "argument to 'save()' must be of type 'str'"))
    }
}

fn canvas_draw_rect(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 7 {
        return Err(error("R014", format!("draw_rect() takes exactly 7 arguments, got {}", args.len())));
    }
    if let (
        Value::Int(start_x), Value::Int(start_y),
        Value::Int(w), Value::Int(h),
        Value::Int(r), Value::Int(g), Value::Int(b)
    ) = (&args[0], &args[1], &args[2], &args[3], &args[4], &args[5], &args[6]) {
        
        let start_x = *start_x as usize;
        let start_y = *start_y as usize;
        let w = *w as usize;
        let h = *h as usize;

        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                let width = *cw.borrow();
                let height = *ch.borrow();

                CANVAS_PIXELS.with(|cp| {
                    let mut pixels = cp.borrow_mut();
                    for y in start_y..(start_y + h) {
                        for x in start_x..(start_x + w) {
                            if x < width && y < height {
                                let idx = (y * width + x) * 3;
                                pixels[idx] = *r as u8;
                                pixels[idx + 1] = *g as u8;
                                pixels[idx + 2] = *b as u8;
                            }
                        }
                    }
                });
            })
        });

        Ok(Value::None)
    } else {
        Err(error("R002", "arguments to 'draw_rect()' must be of type 'int'"))
    }
}

fn canvas_draw_circle(args: &[Value]) -> Result<Value, VyprError> {
    if args.len() != 6 {
        return Err(error("R014", format!("draw_circle() takes exactly 6 arguments, got {}", args.len())));
    }
    if let (
        Value::Int(cx), Value::Int(cy),
        Value::Int(radius),
        Value::Int(r), Value::Int(g), Value::Int(b)
    ) = (&args[0], &args[1], &args[2], &args[3], &args[4], &args[5]) 
    {
        let cx = *cx as isize;
        let cy = *cy as isize;
        let rad = *radius as isize;
        let rad_sq = rad * rad;

        CANVAS_WIDTH.with(|cw| {
            CANVAS_HEIGHT.with(|ch| {
                let width = *cw.borrow() as isize;
                let height = *ch.borrow() as isize;

                CANVAS_PIXELS.with(|cp| {
                    let mut pixels = cp.borrow_mut();
                    
                    // Bounding box optimization
                    let min_y = (cy - rad).max(0);
                    let max_y = (cy + rad).min(height - 1);
                    let min_x = (cx - rad).max(0);
                    let max_x = (cx + rad).min(width - 1);

                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            let dx = x - cx;
                            let dy = y - cy;
                            
                            // Distance formula to check if inside circle
                            if dx * dx + dy * dy <= rad_sq {
                                let idx = ((y as usize) * (width as usize) + (x as usize)) * 3;
                                pixels[idx] = *r as u8;
                                pixels[idx + 1] = *g as u8;
                                pixels[idx + 2] = *b as u8;
                            }
                        }
                    }
                });
            })
        });

        Ok(Value::None)
    } else {
        Err(error("R002", "arguments to 'draw_circle()' must be of type 'int'"))
    }
}
