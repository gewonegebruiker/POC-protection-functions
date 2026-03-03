#!/usr/bin/env bash
# setup-seapath-host.sh — SEAPATH host preparation for RT protection IEDs
#
# Run as root on the hypervisor before deploying containers.
# Idempotent where possible; safe to re-run.

set -euo pipefail

NIC="${NIC:-eth0}"              # Override with: NIC=ens3f0 ./setup-seapath-host.sh
ISOLATED_CORES="${ISOLATED_CORES:-2,3,4}"  # Cores reserved for IEDs and PTP
IRQ_CORE="${IRQ_CORE:-0}"       # Core that handles NIC IRQs

# ---------------------------------------------------------------------------
# 1. Verify PREEMPT_RT kernel
# ---------------------------------------------------------------------------
echo "=== Checking kernel ==="
KERNEL_VERSION=$(uname -r)
echo "Kernel: ${KERNEL_VERSION}"

if uname -v | grep -q "PREEMPT_RT"; then
    echo "✓ PREEMPT_RT kernel detected"
else
    echo "⚠ WARNING: PREEMPT_RT not detected in 'uname -v'."
    echo "  A PREEMPT_RT patched kernel is required for deterministic latency."
    echo "  Check https://cdn.kernel.org/pub/linux/kernel/projects/rt/ for patches."
fi

# ---------------------------------------------------------------------------
# 2. Recommended GRUB boot parameters
# ---------------------------------------------------------------------------
echo ""
echo "=== Recommended GRUB boot parameters ==="
cat <<'EOF'
Add the following to GRUB_CMDLINE_LINUX in /etc/default/grub, then run
  update-grub && reboot

  isolcpus=2,3,4
  nohz_full=2,3,4
  rcu_nocbs=2,3,4
  rcu_nocb_poll
  irqaffinity=0,1
  nosoftlockup
  processor.max_cstate=1
  intel_idle.max_cstate=0
  intel_pstate=disable
  mce=off

NOTE: Adjust CPU numbers to match your hardware topology.
EOF

# ---------------------------------------------------------------------------
# 3. CPU governor — set to performance on isolated cores
# ---------------------------------------------------------------------------
echo ""
echo "=== Setting CPU governor to performance ==="
for cpu in $(echo "${ISOLATED_CORES}" | tr ',' ' '); do
    GOVERNOR_PATH="/sys/devices/system/cpu/cpu${cpu}/cpufreq/scaling_governor"
    if [ -f "${GOVERNOR_PATH}" ]; then
        echo performance > "${GOVERNOR_PATH}"
        echo "  cpu${cpu}: performance"
    else
        echo "  cpu${cpu}: cpufreq not available (may be handled by BIOS/UEFI)"
    fi
done

# ---------------------------------------------------------------------------
# 4. Configure SR-IOV VFs on NIC
# ---------------------------------------------------------------------------
echo ""
echo "=== Configuring SR-IOV on ${NIC} ==="
SRIOV_PATH="/sys/class/net/${NIC}/device/sriov_numvfs"
if [ -f "${SRIOV_PATH}" ]; then
    NUM_VFS=4
    echo "${NUM_VFS}" > "${SRIOV_PATH}"
    echo "  Created ${NUM_VFS} VFs on ${NIC}"
    echo "  Available VFs:"
    ls /sys/class/net/ | grep -E "^${NIC}v" || echo "  (none visible yet — may need driver reload)"
else
    echo "  SR-IOV not supported on ${NIC} or driver not loaded"
fi

# ---------------------------------------------------------------------------
# 5. Verify PTP hardware timestamping
# ---------------------------------------------------------------------------
echo ""
echo "=== Verifying PTP hardware timestamping on ${NIC} ==="
if command -v ethtool &>/dev/null; then
    if ethtool -T "${NIC}" 2>/dev/null | grep -q "hardware-transmit"; then
        echo "  ✓ Hardware PTP timestamping supported"
        ethtool -T "${NIC}" 2>/dev/null | grep -E "(Capabilities|hardware)" || true
    else
        echo "  ⚠ Hardware PTP timestamping NOT detected on ${NIC}"
        echo "    Software timestamping only — latency accuracy will be reduced"
    fi
else
    echo "  ethtool not installed — cannot verify PTP timestamping"
fi

# ---------------------------------------------------------------------------
# 6. Configure irqbalance to avoid isolated cores
# ---------------------------------------------------------------------------
echo ""
echo "=== Configuring irqbalance ==="
IRQBALANCE_CONF="/etc/default/irqbalance"
if [ -f "${IRQBALANCE_CONF}" ]; then
    # Convert comma-separated core list to a hex bitmask for IRQBALANCE_BANNED_CPUS
    # e.g. "2,3,4" → binary 0b11100 → hex "1c"
    BANNED_MASK=0
    for cpu in $(echo "${ISOLATED_CORES}" | tr ',' ' '); do
        BANNED_MASK=$(( BANNED_MASK | (1 << cpu) ))
    done
    BANNED_HEX=$(printf "%x" "${BANNED_MASK}")

    if ! grep -q "IRQBALANCE_BANNED_CPUS" "${IRQBALANCE_CONF}"; then
        echo "IRQBALANCE_BANNED_CPUS=${BANNED_HEX}" >> "${IRQBALANCE_CONF}"
        echo "  Added IRQBALANCE_BANNED_CPUS=${BANNED_HEX} to ${IRQBALANCE_CONF}"
    else
        echo "  IRQBALANCE_BANNED_CPUS already configured"
    fi
    systemctl restart irqbalance 2>/dev/null || true
else
    echo "  irqbalance config not found — install irqbalance or configure manually"
fi

# ---------------------------------------------------------------------------
# 7. Steer NIC interrupts to core 0
# ---------------------------------------------------------------------------
echo ""
echo "=== Steering NIC (${NIC}) interrupts to core ${IRQ_CORE} ==="
AFFINITY_MASK=$(printf "%x" $((1 << IRQ_CORE)))
for irq_dir in /proc/irq/*/; do
    if [ -f "${irq_dir}/../$(basename "${irq_dir}")/smp_affinity" ] 2>/dev/null; then
        true
    fi
done

# Use /proc/interrupts to find IRQ numbers for the NIC
NIC_IRQS=$(grep "${NIC}" /proc/interrupts 2>/dev/null | awk -F: '{print $1}' | tr -d ' ' || true)
if [ -n "${NIC_IRQS}" ]; then
    for irq in ${NIC_IRQS}; do
        AFFINITY_FILE="/proc/irq/${irq}/smp_affinity"
        if [ -f "${AFFINITY_FILE}" ]; then
            echo "${AFFINITY_MASK}" > "${AFFINITY_FILE}" 2>/dev/null || true
            echo "  IRQ ${irq} → cpu${IRQ_CORE} (mask 0x${AFFINITY_MASK})"
        fi
    done
else
    echo "  No IRQs found for ${NIC} in /proc/interrupts"
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo ""
echo "=== Host setup complete ==="
echo "Reboot with recommended GRUB parameters to activate CPU isolation."
