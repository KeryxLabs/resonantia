<script lang="ts">
  export let size = 200;
  export let glow = 'soft';
  export let animated = true;
</script>

<div class="stone-wrap {glow}" class:animated class:still={!animated} style={`width:${size}px;height:${size}px`} aria-hidden="true">
  <svg viewBox="0 0 200 200" class="stone-svg" xmlns="http://www.w3.org/2000/svg">
    <defs>
      <radialGradient id="alkahestCore" cx="42%" cy="36%" r="62%">
        <stop offset="0%" stop-color="#b9dcff" stop-opacity="0.98" />
        <stop offset="40%" stop-color="#5d95d6" stop-opacity="0.95" />
        <stop offset="78%" stop-color="#223c63" stop-opacity="0.95" />
        <stop offset="100%" stop-color="#0a1628" stop-opacity="0.98" />
      </radialGradient>

      <filter id="alkahestGoo">
        <feGaussianBlur stdDeviation="2.4" result="blur" />
        <feColorMatrix
          in="blur"
          mode="matrix"
          values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 22 -7"
          result="goo"
        />
      </filter>
    </defs>

    <g class="ritual-ring" transform="translate(100, 160) scale(1.34, 0.28) rotate(-15)" opacity="0.24">
      <circle r="84" fill="none" stroke="rgba(167, 208, 247, 0.76)" stroke-width="1.25" />
      <polygon points="0,-84 72,41 -72,41" fill="none" stroke="rgba(138, 189, 243, 0.56)" stroke-width="0.9" />
    </g>

    <g class="fluid" filter="url(#alkahestGoo)">
      <path
        fill="url(#alkahestCore)"
        d="M 30,160 Q 50,130 100,145 T 180,160 Q 150,180 100,175 T 30,160"
      />
    </g>
  </svg>
</div>

<style>
  .stone-wrap {
    position: relative;
    display: grid;
    place-items: center;
  }

  .stone-wrap.animated {
    animation: stone-float 18s ease-in-out infinite;
  }

  .stone-wrap.still {
    animation: none;
  }

  .stone-wrap::after {
    content: '';
    position: absolute;
    left: 14%;
    right: 14%;
    bottom: 6%;
    height: 22%;
    border-radius: 999px;
    background: radial-gradient(circle at 50% 50%, rgba(118, 175, 241, 0.36), rgba(12, 22, 36, 0));
    filter: blur(8px);
    transform: scaleX(1.1);
    pointer-events: none;
  }

  .stone-wrap.soft {
    filter: drop-shadow(0 0 18px rgba(108, 170, 241, 0.24));
  }

  .stone-wrap.strong {
    filter:
      drop-shadow(0 0 24px rgba(108, 170, 241, 0.34))
      drop-shadow(0 0 42px rgba(94, 148, 215, 0.25));
  }

  .stone-svg {
    width: 100%;
    height: 100%;
    display: block;
    overflow: visible;
  }

  .ritual-ring {
    transform-origin: 100px 160px;
  }

  .stone-wrap.animated .ritual-ring {
    animation: ring-drift 96s linear infinite;
  }

  .stone-wrap.animated .fluid {
    transform-origin: 100px 160px;
    animation: fluid-breathe 22s ease-in-out infinite;
  }

  @keyframes stone-float {
    0%,
    100% {
      transform: translateY(0px) rotate(-0.25deg);
    }
    50% {
      transform: translateY(-2px) rotate(0.25deg);
    }
  }

  @keyframes ring-drift {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes fluid-breathe {
    0%,
    100% {
      transform: scale(1) translateY(0);
    }
    50% {
      transform: scale(1.012) translateY(-1px);
    }
  }
</style>
