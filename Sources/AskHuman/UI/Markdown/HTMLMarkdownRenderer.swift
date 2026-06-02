import Foundation
import Markdown

/// 把 Markdown 转换为 HTML 片段（基于 swift-markdown AST）
struct HTMLMarkdownRenderer: MarkupVisitor {
    typealias Result = String

    static func html(from markdown: String) -> String {
        var renderer = HTMLMarkdownRenderer()
        let document = Document(parsing: markdown)
        return renderer.visit(document)
    }

    mutating func defaultVisit(_ markup: Markup) -> String {
        renderChildren(markup)
    }

    private mutating func renderChildren(_ markup: Markup) -> String {
        markup.children.map { visit($0) }.joined()
    }

    mutating func visitDocument(_ document: Document) -> String {
        renderChildren(document)
    }

    mutating func visitText(_ text: Markdown.Text) -> String {
        Self.escape(text.string)
    }

    mutating func visitParagraph(_ paragraph: Paragraph) -> String {
        "<p>\(renderChildren(paragraph))</p>"
    }

    mutating func visitHeading(_ heading: Heading) -> String {
        let level = min(max(heading.level, 1), 6)
        return "<h\(level)>\(renderChildren(heading))</h\(level)>"
    }

    mutating func visitEmphasis(_ emphasis: Emphasis) -> String {
        "<em>\(renderChildren(emphasis))</em>"
    }

    mutating func visitStrong(_ strong: Strong) -> String {
        "<strong>\(renderChildren(strong))</strong>"
    }

    mutating func visitStrikethrough(_ strikethrough: Strikethrough) -> String {
        "<del>\(renderChildren(strikethrough))</del>"
    }

    mutating func visitInlineCode(_ inlineCode: InlineCode) -> String {
        "<code>\(Self.escape(inlineCode.code))</code>"
    }

    mutating func visitCodeBlock(_ codeBlock: CodeBlock) -> String {
        let lang = codeBlock.language.map { " class=\"language-\(Self.escape($0))\"" } ?? ""
        return "<pre><code\(lang)>\(Self.escape(codeBlock.code))</code></pre>"
    }

    mutating func visitLink(_ link: Markdown.Link) -> String {
        let href = link.destination.map { Self.escape($0) } ?? "#"
        return "<a href=\"\(href)\">\(renderChildren(link))</a>"
    }

    mutating func visitImage(_ image: Markdown.Image) -> String {
        let src = image.source.map { Self.escape($0) } ?? ""
        let alt = image.plainText
        return "<img src=\"\(src)\" alt=\"\(Self.escape(alt))\" />"
    }

    mutating func visitBlockQuote(_ blockQuote: BlockQuote) -> String {
        "<blockquote>\(renderChildren(blockQuote))</blockquote>"
    }

    mutating func visitUnorderedList(_ unorderedList: UnorderedList) -> String {
        "<ul>\(renderChildren(unorderedList))</ul>"
    }

    mutating func visitOrderedList(_ orderedList: OrderedList) -> String {
        "<ol>\(renderChildren(orderedList))</ol>"
    }

    mutating func visitListItem(_ listItem: ListItem) -> String {
        "<li>\(renderChildren(listItem))</li>"
    }

    mutating func visitThematicBreak(_ thematicBreak: ThematicBreak) -> String {
        "<hr />"
    }

    mutating func visitSoftBreak(_ softBreak: SoftBreak) -> String {
        " "
    }

    mutating func visitLineBreak(_ lineBreak: LineBreak) -> String {
        "<br />"
    }

    mutating func visitInlineHTML(_ inlineHTML: InlineHTML) -> String {
        Self.escape(inlineHTML.rawHTML)
    }

    mutating func visitHTMLBlock(_ html: HTMLBlock) -> String {
        Self.escape(html.rawHTML)
    }

    mutating func visitTable(_ table: Markdown.Table) -> String {
        var rows = "<thead><tr>"
        for cell in table.head.cells {
            rows += "<th>\(renderChildren(cell))</th>"
        }
        rows += "</tr></thead><tbody>"
        for row in table.body.rows {
            rows += "<tr>"
            for cell in row.cells {
                rows += "<td>\(renderChildren(cell))</td>"
            }
            rows += "</tr>"
        }
        rows += "</tbody>"
        return "<table>\(rows)</table>"
    }

    mutating func visitTableCell(_ tableCell: Markdown.Table.Cell) -> String {
        renderChildren(tableCell)
    }

    static func escape(_ text: String) -> String {
        var result = ""
        result.reserveCapacity(text.count)
        for ch in text {
            switch ch {
            case "&": result += "&amp;"
            case "<": result += "&lt;"
            case ">": result += "&gt;"
            case "\"": result += "&quot;"
            case "'": result += "&#39;"
            default: result.append(ch)
            }
        }
        return result
    }
}
