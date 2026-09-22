<script lang="ts">
  import type { Snippet } from "svelte";
  import { T, useTask, useThrelte } from "@threlte/core";
  import { Group, Quaternion } from "three";

  let { position, children }: { position: [number, number, number]; children: Snippet } = $props();
  const group = new Group();
  const parentRotation = new Quaternion();
  const { camera, renderStage, autoRenderTask } = useThrelte();

  // Orient labels only when a frame is already needed. The stock Billboard's
  // auto-invalidating task keeps the entire scene rendering while idle.
  useTask(
    () => {
      $camera.getWorldQuaternion(group.quaternion);
      if (group.parent) {
        group.parent.getWorldQuaternion(parentRotation);
        group.quaternion.premultiply(parentRotation.invert());
      }
    },
    { stage: renderStage, before: autoRenderTask, autoInvalidate: false },
  );
</script>

<T is={group} {position}>
  {@render children()}
</T>
