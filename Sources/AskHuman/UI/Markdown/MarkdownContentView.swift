import SwiftUI
import Markdown

/// 基于 swift-markdown 的块级 Markdown 渲染视图
struct MarkdownContentView: View {
    let markdown: String

    var body: some View {
        let document = Document(parsing: markdown)
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(document.blockChildren.enumerated()), id: \.offset) { _, block in
                MarkdownBlock.render(block)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

enum MarkdownBlock {
    static func render(_ markup: Markup) -> AnyView {
        switch markup {
        case let heading as Heading:
            return AnyView(
                Text(inline(heading))
                    .font(headingFont(level: heading.level))
                    .fontWeight(.bold)
                    .fixedSize(horizontal: false, vertical: true)
            )

        case let paragraph as Paragraph:
            return AnyView(
                Text(inline(paragraph))
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            )

        case let codeBlock as CodeBlock:
            return AnyView(codeBlockView(codeBlock.code))

        case let quote as BlockQuote:
            return AnyView(
                HStack(spacing: 8) {
                    Rectangle()
                        .fill(Color.secondary.opacity(0.4))
                        .frame(width: 3)
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(Array(quote.blockChildren.enumerated()), id: \.offset) { _, child in
                            render(child)
                        }
                    }
                    .foregroundStyle(.secondary)
                }
            )

        case let list as UnorderedList:
            return AnyView(listView(items: Array(list.listItems), ordered: false, start: 1))

        case let list as OrderedList:
            return AnyView(listView(items: Array(list.listItems), ordered: true, start: 1))

        case is ThematicBreak:
            return AnyView(Divider().padding(.vertical, 2))

        case let table as Markdown.Table:
            return AnyView(tableView(table))

        default:
            return AnyView(
                Text(inline(markup))
                    .fixedSize(horizontal: false, vertical: true)
            )
        }
    }

    // MARK: - 列表

    private static func listView(items: [ListItem], ordered: Bool, start: Int) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                HStack(alignment: .top, spacing: 6) {
                    Text(ordered ? "\(start + index)." : "•")
                        .frame(minWidth: 16, alignment: .trailing)
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(item.blockChildren.enumerated()), id: \.offset) { _, child in
                            render(child)
                        }
                    }
                }
            }
        }
    }

    // MARK: - 代码块

    private static func codeBlockView(_ code: String) -> some View {
        let trimmed = code.hasSuffix("\n") ? String(code.dropLast()) : code
        return ScrollView(.horizontal, showsIndicators: false) {
            Text(trimmed)
                .font(.system(.callout, design: .monospaced))
                .textSelection(.enabled)
                .padding(10)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(Color.secondary.opacity(0.12))
        )
    }

    // MARK: - 表格

    private static func tableView(_ table: Markdown.Table) -> some View {
        let headCells = Array(table.head.cells)
        let rows = Array(table.body.rows)
        return VStack(alignment: .leading, spacing: 0) {
            // 表头
            HStack(spacing: 0) {
                ForEach(Array(headCells.enumerated()), id: \.offset) { _, cell in
                    Text(inline(cell))
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(6)
                }
            }
            .background(Color.secondary.opacity(0.12))
            Divider()
            // 表体
            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                let cells = Array(row.cells)
                HStack(spacing: 0) {
                    ForEach(Array(cells.enumerated()), id: \.offset) { _, cell in
                        Text(inline(cell))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(6)
                    }
                }
                Divider()
            }
        }
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(Color.secondary.opacity(0.25))
        )
    }

    // MARK: - 行内

    static func inline(_ markup: Markup) -> AttributedString {
        var out = AttributedString()
        for child in markup.children {
            appendInline(child, into: &out, intent: [], link: nil)
        }
        return out
    }

    private static func appendInline(
        _ markup: Markup,
        into out: inout AttributedString,
        intent: InlinePresentationIntent,
        link: URL?
    ) {
        switch markup {
        case let text as Markdown.Text:
            out += styled(text.string, intent: intent, link: link)

        case let code as InlineCode:
            out += styled(code.code, intent: intent.union(.code), link: link)

        case let strong as Strong:
            for c in strong.children {
                appendInline(c, into: &out, intent: intent.union(.stronglyEmphasized), link: link)
            }

        case let emphasis as Emphasis:
            for c in emphasis.children {
                appendInline(c, into: &out, intent: intent.union(.emphasized), link: link)
            }

        case let strike as Strikethrough:
            for c in strike.children {
                appendInline(c, into: &out, intent: intent.union(.strikethrough), link: link)
            }

        case let anchor as Markdown.Link:
            let url = anchor.destination.flatMap { URL(string: $0) }
            for c in anchor.children {
                appendInline(c, into: &out, intent: intent, link: url ?? link)
            }

        case is SoftBreak:
            out += AttributedString(" ")

        case is LineBreak:
            out += AttributedString("\n")

        default:
            for c in markup.children {
                appendInline(c, into: &out, intent: intent, link: link)
            }
        }
    }

    private static func styled(
        _ string: String,
        intent: InlinePresentationIntent,
        link: URL?
    ) -> AttributedString {
        var piece = AttributedString(string)
        if !intent.isEmpty {
            piece.inlinePresentationIntent = intent
        }
        if let link {
            piece.link = link
            piece.foregroundColor = .accentColor
        }
        return piece
    }

    private static func headingFont(level: Int) -> Font {
        switch level {
        case 1: return .title
        case 2: return .title2
        case 3: return .title3
        default: return .headline
        }
    }
}
