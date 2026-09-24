<script lang="ts">
  import { useThrelte } from "@threlte/core";
  import { PerspectiveCamera, Raycaster, Vector2 } from "three";
  import { cameraPlanePoint, orbitMap, orbitRadiansPerPixel, panMap, zoomMap } from "./map-camera";

  let { flat, radius }: { flat: boolean; radius: number } = $props();
  const { camera, renderer, scene, invalidate } = useThrelte();

  $effect(() => {
    const selectedCamera = $camera;
    if (!(selectedCamera instanceof PerspectiveCamera)) return;
    const activeCamera: PerspectiveCamera = selectedCamera;
    const isFlat = flat;
    const mapRadius = Math.max(1, radius);
    const element = renderer.domElement;
    // Gesture bookkeeping is deliberately non-reactive.
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const pointers = new Map<number, { x: number; y: number; button: number }>();
    let gestureDistance = 0;
    let suppressClick = false;
    const oldTouchAction = element.style.touchAction;
    element.style.touchAction = "none";
    activeCamera.lookAt(0, 0, 0);
    activeCamera.updateMatrixWorld();
    invalidate();

    function ndc(x: number, y: number) {
      const rect = element.getBoundingClientRect();
      return new Vector2(
        ((x - rect.left) / rect.width) * 2 - 1,
        1 - ((y - rect.top) / rect.height) * 2,
      );
    }

    function anchorAt(x: number, y: number) {
      const cursor = ndc(x, y);
      const raycaster = new Raycaster();
      raycaster.setFromCamera(cursor, activeCamera);
      // Pick the visible terrain surface when possible, otherwise the map's center plane.
      const terrain = scene.getObjectByName("terrain");
      const hit = terrain ? raycaster.intersectObject(terrain, false)[0] : undefined;
      return hit?.point ?? cameraPlanePoint(activeCamera, cursor);
    }

    function zoom(x: number, y: number, amount: number) {
      const anchor = anchorAt(x, y);
      if (!anchor) return;
      zoomMap(
        activeCamera,
        anchor,
        Math.exp(amount),
        mapRadius * (isFlat ? 0.03 : 1.05),
        mapRadius * 20,
        !isFlat,
      );
      invalidate();
    }

    function pan(fromX: number, fromY: number, toX: number, toY: number) {
      const anchor = anchorAt(fromX, fromY);
      if (!anchor) return;
      panMap(activeCamera, ndc(fromX, fromY), ndc(toX, toY), anchor);
      invalidate();
    }

    function rotate(dx: number, dy: number) {
      const scale = orbitRadiansPerPixel(activeCamera, mapRadius, element.clientHeight, isFlat);
      orbitMap(activeCamera, dx * scale, dy * scale);
      invalidate();
    }

    function down(event: PointerEvent) {
      if (event.button > 2) return;
      if (pointers.size === 0) {
        gestureDistance = 0;
        suppressClick = false;
      } else suppressClick = true;
      pointers.set(event.pointerId, { x: event.clientX, y: event.clientY, button: event.button });
      element.setPointerCapture(event.pointerId);
    }

    function move(event: PointerEvent) {
      const previous = pointers.get(event.pointerId);
      if (!previous) return;
      gestureDistance += Math.hypot(event.clientX - previous.x, event.clientY - previous.y);
      if (gestureDistance > 8) suppressClick = true;
      const before = [...pointers.values()];
      pointers.set(event.pointerId, {
        x: event.clientX,
        y: event.clientY,
        button: previous.button,
      });
      const after = [...pointers.values()];
      if (before.length === 2) {
        const midpoint = (points: typeof before) =>
          new Vector2((points[0].x + points[1].x) / 2, (points[0].y + points[1].y) / 2);
        const distance = (points: typeof before) =>
          Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y);
        const oldMid = midpoint(before);
        const newMid = midpoint(after);
        const oldDistance = distance(before);
        const newDistance = distance(after);
        if (oldDistance > 0 && newDistance > 0)
          zoom(oldMid.x, oldMid.y, Math.log(oldDistance / newDistance));
        rotate(newMid.x - oldMid.x, newMid.y - oldMid.y);
      } else if (before.length === 1) {
        const dx = event.clientX - previous.x;
        const dy = event.clientY - previous.y;
        if (previous.button === 1) zoom(event.clientX, event.clientY, dy * 0.01);
        else if (event.shiftKey) rotate(dx, dy);
        else pan(previous.x, previous.y, event.clientX, event.clientY);
      }
    }

    function up(event: PointerEvent) {
      pointers.delete(event.pointerId);
      if (element.hasPointerCapture(event.pointerId))
        element.releasePointerCapture(event.pointerId);
    }

    function wheel(event: WheelEvent) {
      event.preventDefault();
      const pixels =
        event.deltaY *
        (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? element.clientHeight : 1);
      zoom(event.clientX, event.clientY, Math.max(-0.5, Math.min(0.5, pixels * 0.001)));
    }

    function click(event: MouseEvent) {
      if (suppressClick) event.stopImmediatePropagation();
    }

    function contextMenu(event: MouseEvent) {
      event.preventDefault();
    }
    element.addEventListener("click", click, true);
    element.addEventListener("pointerdown", down);
    element.addEventListener("pointermove", move);
    element.addEventListener("pointerup", up);
    element.addEventListener("pointercancel", up);
    element.addEventListener("lostpointercapture", up);
    element.addEventListener("wheel", wheel, { passive: false });
    element.addEventListener("contextmenu", contextMenu);
    return () => {
      element.removeEventListener("click", click, true);
      element.removeEventListener("pointerdown", down);
      element.removeEventListener("pointermove", move);
      element.removeEventListener("pointerup", up);
      element.removeEventListener("pointercancel", up);
      element.removeEventListener("lostpointercapture", up);
      element.removeEventListener("wheel", wheel);
      element.removeEventListener("contextmenu", contextMenu);
      for (const id of pointers.keys())
        if (element.hasPointerCapture(id)) element.releasePointerCapture(id);
      element.style.touchAction = oldTouchAction;
    };
  });
</script>
