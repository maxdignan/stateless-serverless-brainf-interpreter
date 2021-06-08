use base64;
use lambda::{handler_fn, Context};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

type Error = Box<dyn std::error::Error + Sync + Send + 'static>;

#[derive(Debug, Serialize, Deserialize)]
struct ProgramContent {
    program_code: String,
    instruction_pointer: u32,
    state: Vec<u8>,
    stdout: String,
    data_pointer: u32,
    expecting_input: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResponseContent {
    serialized_state: String,
    program_code: String,
    stdout: String,
    expecting_input: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda::run(handler_fn(handler)).await?;
    Ok(())
}

async fn handler(event: Value, _: Context) -> Result<Value, Error> {
    let body = &event["body"];
    println!("{}", body);
    let body: Value = serde_json::from_str(body.as_str().unwrap())?;

    let serialized_state = &body["serialized_state"];
    let program_code = &body["program_code"];

    let mut program_content: ProgramContent =
        get_or_start_program_content(serialized_state, program_code);

    if validate_program(&program_content) {
        println!("valid");
        execute_program(&mut program_content, &event["stdin"]);

        let res: ResponseContent = ResponseContent {
            program_code: program_content.program_code.clone(),
            stdout: program_content.stdout.clone(),
            serialized_state: base64::encode(serde_json::to_string(&program_content).unwrap()),
            expecting_input: program_content.expecting_input,
        };

        Ok(serde_json::json!(&res))
    } else {
        Err(Box::from("invalid program"))
    }
}

fn get_or_start_program_content(serialized_state: &Value, program_code: &Value) -> ProgramContent {
    match serialized_state.as_str() {
        Some(s) => {
            if s.len() > 1 {
                serde_json::from_str(s).unwrap()
            } else {
                ProgramContent {
                    program_code: program_code.as_str().unwrap().to_string(),
                    instruction_pointer: 0,
                    state: vec![0x00; 30000],
                    stdout: String::from(""),
                    data_pointer: 0,
                    expecting_input: false,
                }
            }
        }
        None => ProgramContent {
            program_code: program_code.as_str().unwrap().to_string(),
            instruction_pointer: 0,
            state: vec![0x00; 30000],
            stdout: String::from(""),
            data_pointer: 0,
            expecting_input: false,
        },
    }
}

fn execute_program(program_content: &mut ProgramContent, stdin: &Value) {
    // restarting with input from stdin
    if program_content.expecting_input {
        match convert_stdin_to_u8(stdin) {
            Ok(int) => program_content.state[program_content.data_pointer as usize] = int,
            Err(_) => {
                return program_content.stdout =
                    String::from("Valid inputs are either 1..255 or a single character.")
            }
        }

        program_content.expecting_input = false;
    }

    loop {
        let instruction: &str = program_content
            .program_code
            .graphemes(true)
            .collect::<Vec<&str>>()[program_content.instruction_pointer as usize];

        match instruction {
            ">" => program_content.data_pointer = program_content.data_pointer + 1,

            "<" => program_content.data_pointer = program_content.data_pointer - 1,

            "+" => {
                if program_content.state[program_content.data_pointer as usize] == 255 {
                    program_content.state[program_content.data_pointer as usize] = 0
                } else {
                    program_content.state[program_content.data_pointer as usize] =
                        program_content.state[program_content.data_pointer as usize] + 1
                }
            }

            "-" => {
                if program_content.state[program_content.data_pointer as usize] == 0 {
                    program_content.state[program_content.data_pointer as usize] = 255
                } else {
                    program_content.state[program_content.data_pointer as usize] =
                        program_content.state[program_content.data_pointer as usize] - 1
                }
            }

            "." => program_content.stdout.push_str(
                (program_content.state[program_content.data_pointer as usize] as char)
                    .to_string()
                    .as_str(),
            ),

            "," => {
                program_content.expecting_input = true;
                break;
            }

            "[" => {
                if program_content.state[program_content.data_pointer as usize] == 0 {
                    program_content.instruction_pointer =
                        get_matching_forward_instruction_pointer(&program_content);
                } else {
                    ()
                }
            }

            "]" => {
                if program_content.state[program_content.data_pointer as usize] != 0 {
                    program_content.instruction_pointer =
                        get_matching_backward_instruction_pointer(&program_content);
                } else {
                    ()
                }
            }

            _ => panic!("oh no, invalid op found!"),
        }

        program_content.instruction_pointer = program_content.instruction_pointer + 1;
        // program_content.stdout.push_str(program_content.instruction_pointer.to_string().as_str());

        if program_content.instruction_pointer as usize >= program_content.program_code.len() {
            break;
        }
    }
}

fn get_matching_forward_instruction_pointer(program_content: &ProgramContent) -> u32 {
    let mut net_forward_ops = 0;
    let mut local_instr_ptr = program_content.instruction_pointer;

    loop {
        local_instr_ptr = local_instr_ptr + 1;
        let instruction: &str = program_content
            .program_code
            .graphemes(true)
            .collect::<Vec<&str>>()[local_instr_ptr as usize];

        if instruction == "[" {
            net_forward_ops = net_forward_ops + 1;
        } else if instruction == "]" {
            if net_forward_ops == 0 {
                break;
            } else {
                net_forward_ops = net_forward_ops - 1;
            }
        }
    }

    local_instr_ptr
}

fn get_matching_backward_instruction_pointer(program_content: &ProgramContent) -> u32 {
    let mut net_backward_ops = 0;
    let mut local_instr_ptr = program_content.instruction_pointer;

    loop {
        local_instr_ptr = local_instr_ptr - 1;
        let instruction: &str = program_content
            .program_code
            .graphemes(true)
            .collect::<Vec<&str>>()[local_instr_ptr as usize];

        if instruction == "]" {
            net_backward_ops = net_backward_ops + 1;
        } else if instruction == "[" {
            if net_backward_ops == 0 {
                break;
            } else {
                net_backward_ops = net_backward_ops - 1;
            }
        }
    }

    local_instr_ptr
}

fn validate_program(program_content: &ProgramContent) -> bool {
    if program_content.program_code.len() > 0 {
        let re = Regex::new(r"^[\+\-<>\.,\[\]]+$").unwrap();
        re.is_match(program_content.program_code.as_str())
    } else {
        false
    }
}

fn convert_stdin_to_u8(stdin: &Value) -> Result<u8, bool> {
    let stdin_as_string = stdin.as_str().unwrap();
    let possible_u8_int = stdin_as_string.parse::<u8>();

    if possible_u8_int.is_err() {
        if stdin_as_string.len() > 1 {
            println!("Valid inputs are either 1..255 or a single character.");
            Err(false)
        } else {
            Ok(stdin_as_string.chars().nth(0).unwrap() as u8)
        }
    } else {
        Ok(possible_u8_int.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn handler_handles() {
        let json_body = json!({
          "program_code": "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++."
        });
        let stringified_body: String = json_body.to_string();

        let event = json!({ "body": stringified_body });

        let handler_run = handler(event.clone(), Context::default())
            .await
            .expect("expected Ok(_) value");

        // let output_base64_encoded = base64::decode(handler_run).unwrap();
        // let output_as_string: String = output_base64_encoded.into_iter().collect();
        // let exec = serde_json::from_str(output_as_string).unwrap().program_code;

        let val = &handler_run["program_code"];

        println!("{}", val);
        println!("{}", &handler_run["stdout"]);

        assert_eq!(val.as_str().unwrap(), String::from("++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++."));
        assert_eq!(
            handler_run["stdout"].as_str().unwrap(),
            String::from("Hello World!\n")
        )
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_255() {
        let event = json!({
          "stdin": "255"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Ok(255));
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_cap_a() {
        let event = json!({
          "stdin": "A"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Ok(65));
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_a() {
        let event = json!({
          "stdin": "a"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Ok(97));
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_cap_aa() {
        let event = json!({
          "stdin": "AA"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Err(false));
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_256() {
        let event = json!({
          "stdin": "256"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Err(false));
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_0() {
        let event = json!({
          "stdin": "0"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Ok(0));
    }

    #[tokio::test]
    async fn test_convert_stdin_to_u8_neg1() {
        let event = json!({
          "stdin": "-1"
        });

        let stdin_converted = convert_stdin_to_u8(&event["stdin"]);
        assert_eq!(stdin_converted, Err(false));
    }
}
