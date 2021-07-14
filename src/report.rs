use crate::message::CommitType;
use crate::ChangeLog;

pub fn report(changelog: &ChangeLog) {
    if let Some(features) = changelog.get(&CommitType::Feature) {
        println!("✨ New Features\n");
        for message in features {
            println!(
                " - {} {}",
                if message.is_breaking { "💥 " } else { "" },
                message.summary
            );
        }
        println!();
    }

    if let Some(features) = changelog.get(&CommitType::BugFix) {
        println!("🐛 Bug Fixes\n");
        for message in features {
            println!(
                " - {} {}",
                if message.is_breaking { "💥 " } else { "" },
                message.summary
            );
        }
        println!();
    }
}
