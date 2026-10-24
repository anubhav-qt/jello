<script lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";

  let { text, inline = false }: { text: string; inline?: boolean } = $props();

  const options = { async: false, breaks: true, gfm: true } as const;
  let html = $derived.by(() => {
    let rendered: string;
    if (inline) {
      rendered = marked.parseInline(text, options);
    } else {
      rendered = marked.parse(text, options);
    }
    // Model output can contain HTML. Only retain text formatting.
    return DOMPurify.sanitize(rendered, {
      ALLOWED_TAGS: [
        "p",
        "br",
        "strong",
        "em",
        "del",
        "code",
        "pre",
        "ul",
        "ol",
        "li",
        "blockquote",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "a",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
      ],
      ALLOWED_ATTR: ["href", "title", "start"],
      ALLOW_DATA_ATTR: false,
      ALLOW_ARIA_ATTR: false,
    });
  });
</script>

{#if inline}
  <span class="tutor-markdown">{@html html}</span>
{:else}
  <div class="tutor-markdown">{@html html}</div>
{/if}
