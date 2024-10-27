use crate::analysis::{AnalysisCommand, AnalysisSetup, AnalysisVerdict, MailAnalyzer};
use mail_auth::common::verify::VerifySignature;
use mail_auth::{AuthenticatedMessage, DkimResult, DmarcResult, Resolver, SpfOutput, SpfResult};
use mail_parser::{Address, Host, Message, MessageParser};
use rocket::serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

pub struct AuthAnalyzer;

impl MailAnalyzer for AuthAnalyzer {
    fn name(&self) -> String {
        String::from("Authentication Checks")
    }
    
    fn analyze(&self, email: Message, command: AnalysisCommand) -> AnalysisSetup {
        let email_string = std::str::from_utf8(&email.raw_message).unwrap().to_string();
        let resolver = Resolver::new_system_conf().unwrap();

        let command = Arc::new(command);

        macro_rules! wrap_check_task {
            ($fun:expr) => {{
                let resolver = resolver.clone();
                let email_string = email_string.clone();
                command.spawn($fun(resolver, email_string));
            }};
        }

        wrap_check_task!(verify_dkim);
        wrap_check_task!(verify_arc_chain);
        wrap_check_task!(verify_spf);
        wrap_check_task!(verify_dmarc);

        command.gen_setup()
    }
}

#[derive(Serialize)]
struct DKIMVerdict {
    results: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum DKIMAnalysisVerdict {
    Pass,
    Neutral { value: String },
    Fail { value: String },
    PermError { value: String },
    TempError { value: String },
    None,
}

impl From<&DkimResult> for DKIMAnalysisVerdict {
    fn from(value: &DkimResult) -> Self {
        match value {
            DkimResult::Pass => DKIMAnalysisVerdict::Pass,
            DkimResult::Neutral(e) => DKIMAnalysisVerdict::Neutral {
                value: e.to_string(),
            },
            DkimResult::Fail(e) => DKIMAnalysisVerdict::Fail {
                value: e.to_string(),
            },
            DkimResult::PermError(e) => DKIMAnalysisVerdict::PermError {
                value: e.to_string(),
            },
            DkimResult::TempError(e) => DKIMAnalysisVerdict::TempError {
                value: e.to_string(),
            },
            DkimResult::None => DKIMAnalysisVerdict::None,
        }
    }
}

async fn verify_dkim(resolver: Resolver, msg: String) -> AnalysisVerdict {
    let msg = AuthenticatedMessage::parse(msg.as_bytes()).unwrap();

    let outputs = resolver
        .verify_dkim(&msg)
        .await
        .iter()
        .map(|output| {
            let domain = output.signature().unwrap().domain().to_string();
            let value = DKIMAnalysisVerdict::from(output.result());
            (domain, value)
        })
        .collect::<HashMap<_, _>>();

    AnalysisVerdict::new("auth-dkim", &outputs)
}

async fn verify_arc_chain(resolver: Resolver, msg: String) -> AnalysisVerdict {
    let msg = AuthenticatedMessage::parse(msg.as_bytes()).unwrap();

    let result = resolver.verify_arc(&msg).await;

    AnalysisVerdict::new(
        "auth-arc-chain",
        &DKIMAnalysisVerdict::from(result.result()),
    )
}

#[derive(Serialize)]
struct SpfAnalysisVerdict {
    domain: String,
    result: String,
}

async fn verify_spf(resolver: Resolver, msg: String) -> AnalysisVerdict {
    let spf_output = check_spf(resolver, msg).await;

    let result = match spf_output.result() {
        SpfResult::Pass => "pass",
        SpfResult::Fail => "fail",
        SpfResult::SoftFail => "softfail",
        SpfResult::Neutral => "neutral",
        SpfResult::TempError | SpfResult::PermError => "error",
        SpfResult::None => "unknown",
    };

    AnalysisVerdict::new(
        "auth-spf",
        &SpfAnalysisVerdict {
            domain: spf_output.domain().to_string(),
            result: result.to_string(),
        },
    )
}

async fn check_spf(resolver: Resolver, msg: String) -> SpfOutput {
    let msg = MessageParser::new().parse(&msg).unwrap();

    let sender_email_address = match msg.from().unwrap() {
        Address::List(l) => l[0].address().unwrap(),
        Address::Group(g) => g[0].addresses[0].address().unwrap(),
    };

    let received = msg.received().unwrap();

    let Some(sender_ip) = received.from_ip else {
        return SpfOutput::default()
    };
    let sender_host_domain = match received.from() {
        None => return SpfOutput::default(),
        Some(Host::Name(domain)) => domain.to_string(),
        Some(Host::IpAddr(addr)) => addr.to_string(),
    };

    let helo_domain = match received.helo() {
        Some(Host::Name(domain)) => domain.to_string(),
        Some(Host::IpAddr(addr)) => addr.to_string(),
        None => return SpfOutput::default(),
    };

    resolver
        .verify_spf_sender(
            sender_ip,
            &helo_domain,
            &sender_host_domain,
            &sender_email_address,
        )
        .await
}

#[derive(Serialize)]
struct DmarcAnalysisVerdict {
    dkim: String,
    spf: String,
}

async fn verify_dmarc(resolver: Resolver, msg_string: String) -> AnalysisVerdict {
    let spf_result = check_spf(resolver.clone(), msg_string.clone()).await;

    let msg = AuthenticatedMessage::parse(msg_string.as_bytes()).unwrap();
    let dkim_result = resolver.verify_dkim(&msg).await;

    let sender_email_address = msg.from();
    let sender_email_domain = sender_email_address.rsplit('@').next().unwrap();

    let dmarc_output = resolver
        .verify_dmarc(&msg, &dkim_result, sender_email_domain, &spf_result, |d| {
            psl::domain_str(d).unwrap_or(d)
        })
        .await;

    let dkim_value = match dmarc_output.dkim_result() {
        DmarcResult::Pass => "pass",
        DmarcResult::Fail(e) => "fail",
        DmarcResult::TempError(e) | DmarcResult::PermError(e) => "error",
        DmarcResult::None => "unknown",
    };

    let spf_value = match dmarc_output.spf_result() {
        DmarcResult::Pass => "pass",
        DmarcResult::Fail(e) => "fail",
        DmarcResult::TempError(e) | DmarcResult::PermError(e) => "error",
        DmarcResult::None => "unknown",
    };

    AnalysisVerdict::new(
        "auth-dmarc",
        &DmarcAnalysisVerdict {
            dkim: dkim_value.to_string(),
            spf: spf_value.to_string(),
        },
    )
}
