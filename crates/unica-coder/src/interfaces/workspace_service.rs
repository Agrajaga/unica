use crate::infrastructure::workspace_services;

pub fn run_from_args(args: &[String]) -> Result<(), String> {
    workspace_services::run_workspace_service_from_args(args)
}
