use std::fmt::Write;

use crate::error::Result;
use crate::message::CommitType;
use crate::ChangeLog;

pub fn build_report(output: &mut dyn Write, changelog: &ChangeLog) -> Result<()> {
    if let Some(features) = changelog.get(&CommitType::Feature) {
        writeln!(output, "    ✨ New Features\n")?;
        for message in features {
            writeln!(
                output,
                "       - {} {}",
                if message.is_breaking { "💥 " } else { "" },
                message.summary
            )?;
        }
        writeln!(output)?;
    }

    if let Some(features) = changelog.get(&CommitType::BugFix) {
        writeln!(output, "    🐛 Bug Fixes\n")?;
        for message in features {
            writeln!(
                output,
                "       - {} {}",
                if message.is_breaking { "💥 " } else { "" },
                message.summary
            )?;
        }
        writeln!(output)?;
    }

    Ok(())
}
