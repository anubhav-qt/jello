<!-- "Listens to" on a Mix effect page (docs/design.md#mix): its own input,
     or another track or a bus, the sidechain. -->
<script lang="ts">
  import { listenPlaces, type MixSender, type MixView, type SlotView } from "./mix";
  import MixChoice from "./MixChoice.svelte";

  interface Props {
    place: string;
    slot: number;
    effect: SlotView;
    view: MixView;
    send: MixSender;
    label?: string;
  }

  let { place, slot, effect, view, send, label = "Listens to" }: Props = $props();

  // What a tile's click sends: "own", "track 2" or "bus 1".
  function chooser(word: string): () => void {
    return () => send.setEffectValue(place, slot, "listens_to", word);
  }

  let tiles = $derived.by(() => {
    const now = effect.values.listens_to;
    const all = [{ name: "Own", chosen: now === "own", title: "Its own input", onClick: chooser("own") }];
    for (const each of listenPlaces(view, place)) {
      let title = each.word;
      if (each.word.startsWith("track ")) {
        title = `Track ${each.name}: ${view.tracks[Number(each.name) - 1]?.name ?? ""}`;
      } else {
        const bus = Number(each.word.split(" ")[1]);
        title = `Bus ${bus}: ${view.buses[bus - 1]?.name ?? ""}`;
      }
      all.push({ name: each.name, chosen: now === each.word, title, onClick: chooser(each.word) });
    }
    return all;
  });
</script>

<div class="mix-listens">
  <MixChoice {tiles} {label} />
</div>
