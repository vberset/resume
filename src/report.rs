use std::fmt::Write;

use crate::{
    error::Result,
    message::{CommitType, ConventionalMessage},
    ChangeLog,
};

pub fn build_report<W: Write>(output: &mut W, changelog: &ChangeLog) -> Result<()> {
    let categories = [
        (CommitType::Feature, '✨', "New Features"),
        (CommitType::BugFix, '🐛', "Bug Fixes"),
        (CommitType::Refactoring, '♻', "Refactoring"),
    ];

    for category in categories {
        if let Some(messages) = changelog.get(&category.0) {
            format_category(output, category.1, category.2, messages)?;
        }
    }

    Ok(())
}

fn format_category<W: Write>(
    output: &mut W,
    emoji: char,
    title: &str,
    messages: &[ConventionalMessage],
) -> Result<()> {
    writeln!(output, "    {}️ {}\n", emoji, title)?;
    for message in messages {
        writeln!(
            output,
            "       - {} {}",
            if message.is_breaking { "💥 " } else { "" },
            message.summary
        )?;
    }
    writeln!(output)?;
    Ok(())
}
