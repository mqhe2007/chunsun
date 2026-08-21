<script setup lang="ts">
import InvitationCodesView from "@/screens/admin/InvitationCodesView.vue";

definePage({
  meta: { title: "邀请码", requiresAdmin: true },
});
</script>

<template>
  <InvitationCodesView />
</template>
