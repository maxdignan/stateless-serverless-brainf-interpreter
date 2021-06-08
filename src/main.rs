use lambda::{handler_fn, Context};
use regex::Regex;
use serde_json::{json, Value};
use serde::{Serialize, Deserialize};
use base64;

type Error = Box<dyn std::error::Error + Sync + Send + 'static>;

#[derive(Debug, Serialize, Deserialize)]
struct ProgramContent {
    program_code: String,
    instruction_pointer: u64,
    state: Vec<u8>,
    stdout: String
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda::run(handler_fn(handler)).await?;
    Ok(())
}

async fn handler(event: Value, _: Context) -> Result<Value, Error> {
    let serialized_state = &event["serialized_state"];
    let program_code = &event["program_code"];

    let mut program_content: ProgramContent = get_or_start_program_content(serialized_state, program_code);

    if validate_program(&program_content) {
        println!("valid");
        execute_program(&mut program_content, &event["stdin"]);
        Ok(serde_json::value::Value::String(base64::encode(serde_json::to_string(&program_content).unwrap())))
    } else {
        Err(Box::from("invalid program"))
    }
}

fn get_or_start_program_content(serialized_state: &Value, program_code: &Value) -> ProgramContent {
  match serialized_state.as_str() {
    Some(s) =>
      if s.len() > 1 {
        serde_json::from_str(s).unwrap()
      } else {
        ProgramContent {
          program_code: program_code.as_str().unwrap().to_string(),
          instruction_pointer: 0,
          state: vec![0x00; 30000],
          stdout: String::from("")
        }
      }
    None => 
      ProgramContent {
        program_code: program_code.as_str().unwrap().to_string(),
        instruction_pointer: 0,
        state: vec![0x00; 30000],
        stdout: String::from("")
      }
  }
  
}

fn execute_program(program_content: &mut ProgramContent, serialized_state: &Value) {

}

fn validate_program(program_content: &ProgramContent) -> bool {
    if program_content.program_code.len() > 0 {
        let re = Regex::new(r"^[\+\-<>\.,\[\]]+$").unwrap();
        re.is_match(program_content.program_code.as_str())
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn handler_handles() {
        let event = json!({
            "program_code": "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++."
        });
        assert_eq!(
            handler(event.clone(), Context::default())
                .await
                .expect("expected Ok(_) value"),
            String::from("++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.")
        )
    }
}
